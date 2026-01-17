package dev.angzarr.examples.inventory

import dev.angzarr.BusinessLogicGrpcKt
import dev.angzarr.BusinessResponse
import dev.angzarr.ContextualCommand
import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import com.google.protobuf.Timestamp
import examples.Domains.*
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.Status
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("InventoryServer")
private const val DOMAIN = "inventory"
private const val LOW_STOCK_THRESHOLD = 10

class InventoryService : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

    override suspend fun handle(request: ContextualCommand): BusinessResponse {
        val cmdBook = request.command
        val priorEvents = request.events

        if (cmdBook.pagesList.isEmpty()) {
            throw Status.INVALID_ARGUMENT
                .withDescription("CommandBook has no pages")
                .asRuntimeException()
        }

        val cmdPage = cmdBook.pagesList[0]
        val cmd = cmdPage.command ?: throw Status.INVALID_ARGUMENT
            .withDescription("Command page has no command")
            .asRuntimeException()

        val state = rebuildState(priorEvents)

        try {
            val eventBook = when {
                cmd.`is`(InitializeStock::class.java) -> {
                    val c = cmd.unpack(InitializeStock::class.java)
                    handleInitializeStock(state, c.productId, c.quantity)
                }
                cmd.`is`(ReceiveStock::class.java) -> {
                    val c = cmd.unpack(ReceiveStock::class.java)
                    handleReceiveStock(state, c.quantity)
                }
                cmd.`is`(ReserveStock::class.java) -> {
                    val c = cmd.unpack(ReserveStock::class.java)
                    handleReserveStock(state, c.orderId, c.quantity)
                }
                cmd.`is`(ReleaseReservation::class.java) -> {
                    val c = cmd.unpack(ReleaseReservation::class.java)
                    handleReleaseReservation(state, c.orderId)
                }
                cmd.`is`(CommitReservation::class.java) -> {
                    val c = cmd.unpack(CommitReservation::class.java)
                    handleCommitReservation(state, c.orderId)
                }
                else -> throw Status.INVALID_ARGUMENT
                    .withDescription("Unknown command type: ${cmd.typeUrl}")
                    .asRuntimeException()
            }

            val eventBookWithCover = eventBook.toBuilder()
                .setCover(cmdBook.cover)
                .build()

            return BusinessResponse.newBuilder()
                .setEvents(eventBookWithCover)
                .build()

        } catch (e: CommandValidationException) {
            throw Status.fromCode(e.statusCode)
                .withDescription(e.message)
                .asRuntimeException()
        }
    }

    private fun rebuildState(eventBook: EventBook?): InventoryState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return InventoryState.empty()
        }

        var state = InventoryState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: InventoryState, event: Any): InventoryState {
        return when {
            event.`is`(StockInitialized::class.java) -> {
                val e = event.unpack(StockInitialized::class.java)
                state.copy(productId = e.productId, onHand = e.quantity)
            }
            event.`is`(StockReceived::class.java) -> {
                val e = event.unpack(StockReceived::class.java)
                state.copy(onHand = e.newOnHand)
            }
            event.`is`(StockReserved::class.java) -> {
                val e = event.unpack(StockReserved::class.java)
                val newReservations = state.reservations + Reservation(e.orderId, e.quantity)
                state.copy(reserved = e.newReserved, reservations = newReservations)
            }
            event.`is`(ReservationReleased::class.java) -> {
                val e = event.unpack(ReservationReleased::class.java)
                val remainingReservations = state.reservations.filter { it.orderId != e.orderId }
                state.copy(reserved = e.newReserved, reservations = remainingReservations)
            }
            event.`is`(ReservationCommitted::class.java) -> {
                val e = event.unpack(ReservationCommitted::class.java)
                val remainingReservations = state.reservations.filter { it.orderId != e.orderId }
                state.copy(
                    onHand = e.newOnHand,
                    reserved = e.newReserved,
                    reservations = remainingReservations
                )
            }
            else -> state
        }
    }

    private fun handleInitializeStock(state: InventoryState, productId: String, quantity: Int): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Inventory already initialized")
        }
        if (productId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Product ID is required")
        }
        if (quantity < 0) {
            throw CommandValidationException.invalidArgument("Quantity cannot be negative")
        }

        logger.info("initializing_stock product_id={} quantity={}", productId, quantity)

        val event = StockInitialized.newBuilder()
            .setProductId(productId)
            .setQuantity(quantity)
            .build()

        return createEventBook(event)
    }

    private fun handleReceiveStock(state: InventoryState, quantity: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Inventory not initialized")
        }
        if (quantity <= 0) {
            throw CommandValidationException.invalidArgument("Quantity must be positive")
        }

        val newOnHand = state.onHand + quantity

        logger.info("receiving_stock product_id={} quantity={} new_on_hand={}",
            state.productId, quantity, newOnHand)

        val event = StockReceived.newBuilder()
            .setQuantity(quantity)
            .setNewOnHand(newOnHand)
            .build()

        return createEventBook(event)
    }

    private fun handleReserveStock(state: InventoryState, orderId: String, quantity: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Inventory not initialized")
        }
        if (orderId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Order ID is required")
        }
        if (quantity <= 0) {
            throw CommandValidationException.invalidArgument("Quantity must be positive")
        }
        if (state.reservations.any { it.orderId == orderId }) {
            throw CommandValidationException.failedPrecondition("Reservation already exists for order")
        }

        val available = state.available()
        if (quantity > available) {
            throw CommandValidationException.failedPrecondition(
                "Insufficient stock: available=$available, requested=$quantity"
            )
        }

        val newReserved = state.reserved + quantity
        val newAvailable = state.onHand - newReserved

        logger.info("reserving_stock product_id={} order_id={} quantity={} new_available={}",
            state.productId, orderId, quantity, newAvailable)

        val events = mutableListOf<com.google.protobuf.Message>()

        events.add(StockReserved.newBuilder()
            .setOrderId(orderId)
            .setQuantity(quantity)
            .setNewReserved(newReserved)
            .build())

        if (newAvailable < LOW_STOCK_THRESHOLD && available >= LOW_STOCK_THRESHOLD) {
            events.add(LowStockAlert.newBuilder()
                .setProductId(state.productId)
                .setAvailableQuantity(newAvailable)
                .setThreshold(LOW_STOCK_THRESHOLD)
                .build())
        }

        return createEventBook(events)
    }

    private fun handleReleaseReservation(state: InventoryState, orderId: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Inventory not initialized")
        }

        val reservation = state.reservations.find { it.orderId == orderId }
            ?: throw CommandValidationException.failedPrecondition("Reservation not found")

        val newReserved = state.reserved - reservation.quantity

        logger.info("releasing_reservation product_id={} order_id={} quantity={} new_reserved={}",
            state.productId, orderId, reservation.quantity, newReserved)

        val event = ReservationReleased.newBuilder()
            .setOrderId(orderId)
            .setQuantity(reservation.quantity)
            .setNewReserved(newReserved)
            .build()

        return createEventBook(event)
    }

    private fun handleCommitReservation(state: InventoryState, orderId: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Inventory not initialized")
        }

        val reservation = state.reservations.find { it.orderId == orderId }
            ?: throw CommandValidationException.failedPrecondition("Reservation not found")

        val newOnHand = state.onHand - reservation.quantity
        val newReserved = state.reserved - reservation.quantity

        logger.info("committing_reservation product_id={} order_id={} quantity={} new_on_hand={}",
            state.productId, orderId, reservation.quantity, newOnHand)

        val event = ReservationCommitted.newBuilder()
            .setOrderId(orderId)
            .setQuantity(reservation.quantity)
            .setNewOnHand(newOnHand)
            .setNewReserved(newReserved)
            .build()

        return createEventBook(event)
    }

    private fun createEventBook(event: com.google.protobuf.Message): EventBook {
        return createEventBook(listOf(event))
    }

    private fun createEventBook(events: List<com.google.protobuf.Message>): EventBook {
        val builder = EventBook.newBuilder()

        events.forEachIndexed { index, event ->
            val page = EventPage.newBuilder()
                .setNum(index)
                .setEvent(Any.pack(event))
                .setCreatedAt(nowTimestamp())
                .build()
            builder.addPages(page)
        }

        return builder.build()
    }

    private fun nowTimestamp(): Timestamp = Timestamp.newBuilder()
        .setSeconds(System.currentTimeMillis() / 1000)
        .build()
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50504

    val service = InventoryService()
    val health = HealthStatusManager()

    val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .addService(health.healthService)
        .build()
        .start()

    health.setStatus("", HealthCheckResponse.ServingStatus.SERVING)
    logger.info("Business logic server started: domain={}, port={}", DOMAIN, port)

    Runtime.getRuntime().addShutdownHook(Thread {
        server.shutdown()
    })

    server.awaitTermination()
}
