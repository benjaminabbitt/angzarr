package dev.angzarr.examples.fulfillment

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

private val logger = LoggerFactory.getLogger("FulfillmentServer")
private const val DOMAIN = "fulfillment"

class FulfillmentService : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

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
                cmd.`is`(CreateShipment::class.java) -> {
                    val c = cmd.unpack(CreateShipment::class.java)
                    handleCreateShipment(state, c.orderId)
                }
                cmd.`is`(MarkPicked::class.java) -> {
                    handleMarkPicked(state)
                }
                cmd.`is`(MarkPacked::class.java) -> {
                    handleMarkPacked(state)
                }
                cmd.`is`(Ship::class.java) -> {
                    val c = cmd.unpack(Ship::class.java)
                    handleShip(state, c.trackingNumber, c.carrier)
                }
                cmd.`is`(RecordDelivery::class.java) -> {
                    handleRecordDelivery(state)
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

    private fun rebuildState(eventBook: EventBook?): FulfillmentState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return FulfillmentState.empty()
        }

        var state = FulfillmentState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: FulfillmentState, event: Any): FulfillmentState {
        return when {
            event.`is`(ShipmentCreated::class.java) -> {
                val e = event.unpack(ShipmentCreated::class.java)
                state.copy(orderId = e.orderId, status = "pending")
            }
            event.`is`(ItemsPicked::class.java) -> {
                state.copy(status = "picking")
            }
            event.`is`(ItemsPacked::class.java) -> {
                state.copy(status = "packing")
            }
            event.`is`(Shipped::class.java) -> {
                val e = event.unpack(Shipped::class.java)
                state.copy(status = "shipped", trackingNumber = e.trackingNumber)
            }
            event.`is`(Delivered::class.java) -> {
                state.copy(status = "delivered")
            }
            else -> state
        }
    }

    private fun handleCreateShipment(state: FulfillmentState, orderId: String): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Shipment already exists")
        }
        if (orderId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Order ID is required")
        }

        logger.info("creating_shipment order_id={}", orderId)

        val event = ShipmentCreated.newBuilder()
            .setOrderId(orderId)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleMarkPicked(state: FulfillmentState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Shipment does not exist")
        }
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Shipment not in pending state")
        }

        logger.info("marking_picked order_id={}", state.orderId)

        val event = ItemsPicked.newBuilder()
            .setPickedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleMarkPacked(state: FulfillmentState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Shipment does not exist")
        }
        if (!state.isPicking()) {
            throw CommandValidationException.failedPrecondition("Items not picked yet")
        }

        logger.info("marking_packed order_id={}", state.orderId)

        val event = ItemsPacked.newBuilder()
            .setPackedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleShip(state: FulfillmentState, trackingNumber: String, carrier: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Shipment does not exist")
        }
        if (!state.isPacking()) {
            throw CommandValidationException.failedPrecondition("Items not packed yet")
        }
        if (trackingNumber.isEmpty()) {
            throw CommandValidationException.invalidArgument("Tracking number is required")
        }

        logger.info("shipping order_id={} tracking={} carrier={}", state.orderId, trackingNumber, carrier)

        val event = Shipped.newBuilder()
            .setTrackingNumber(trackingNumber)
            .setCarrier(carrier)
            .setShippedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleRecordDelivery(state: FulfillmentState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Shipment does not exist")
        }
        if (!state.isShipped()) {
            throw CommandValidationException.failedPrecondition("Order not shipped yet")
        }

        logger.info("recording_delivery order_id={}", state.orderId)

        val event = Delivered.newBuilder()
            .setDeliveredAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun createEventBook(event: com.google.protobuf.Message): EventBook {
        val page = EventPage.newBuilder()
            .setNum(0)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build()

        return EventBook.newBuilder()
            .addPages(page)
            .build()
    }

    private fun nowTimestamp(): Timestamp = Timestamp.newBuilder()
        .setSeconds(System.currentTimeMillis() / 1000)
        .build()
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50505

    val service = FulfillmentService()
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
