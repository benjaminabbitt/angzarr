package dev.angzarr.examples.cart

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

private val logger = LoggerFactory.getLogger("CartServer")
private const val DOMAIN = "cart"

class CartService : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

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
                cmd.`is`(CreateCart::class.java) -> {
                    val c = cmd.unpack(CreateCart::class.java)
                    handleCreateCart(state, c.customerId)
                }
                cmd.`is`(AddItem::class.java) -> {
                    val c = cmd.unpack(AddItem::class.java)
                    handleAddItem(state, c.productId, c.name, c.quantity, c.unitPriceCents)
                }
                cmd.`is`(UpdateQuantity::class.java) -> {
                    val c = cmd.unpack(UpdateQuantity::class.java)
                    handleUpdateQuantity(state, c.productId, c.quantity)
                }
                cmd.`is`(RemoveItem::class.java) -> {
                    val c = cmd.unpack(RemoveItem::class.java)
                    handleRemoveItem(state, c.productId)
                }
                cmd.`is`(ApplyCoupon::class.java) -> {
                    val c = cmd.unpack(ApplyCoupon::class.java)
                    handleApplyCoupon(state, c.couponCode, c.discountCents)
                }
                cmd.`is`(ClearCart::class.java) -> {
                    handleClearCart(state)
                }
                cmd.`is`(Checkout::class.java) -> {
                    val c = cmd.unpack(Checkout::class.java)
                    handleCheckout(state, c.loyaltyPointsToUse)
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

    private fun rebuildState(eventBook: EventBook?): CartState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return CartState.empty()
        }

        var state = CartState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: CartState, event: Any): CartState {
        return when {
            event.`is`(CartCreated::class.java) -> {
                val e = event.unpack(CartCreated::class.java)
                state.copy(customerId = e.customerId, status = "active")
            }
            event.`is`(ItemAdded::class.java) -> {
                val e = event.unpack(ItemAdded::class.java)
                val newItems = state.items + e.item
                state.copy(items = newItems, subtotalCents = e.newSubtotalCents)
            }
            event.`is`(QuantityUpdated::class.java) -> {
                val e = event.unpack(QuantityUpdated::class.java)
                val updatedItems = state.items.map { item ->
                    if (item.productId == e.productId) {
                        item.toBuilder().setQuantity(e.newQuantity).build()
                    } else item
                }
                state.copy(items = updatedItems, subtotalCents = e.newSubtotalCents)
            }
            event.`is`(ItemRemoved::class.java) -> {
                val e = event.unpack(ItemRemoved::class.java)
                val remainingItems = state.items.filter { it.productId != e.productId }
                state.copy(items = remainingItems, subtotalCents = e.newSubtotalCents)
            }
            event.`is`(CouponApplied::class.java) -> {
                val e = event.unpack(CouponApplied::class.java)
                state.copy(couponCode = e.couponCode, discountCents = e.discountCents)
            }
            event.`is`(CartCleared::class.java) -> {
                state.copy(items = emptyList(), subtotalCents = 0, couponCode = "", discountCents = 0)
            }
            event.`is`(CartCheckoutRequested::class.java) -> {
                state.copy(status = "checked_out")
            }
            else -> state
        }
    }

    private fun handleCreateCart(state: CartState, customerId: String): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart already exists")
        }
        if (customerId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Customer ID is required")
        }

        logger.info("creating_cart customer_id={}", customerId)

        val event = CartCreated.newBuilder()
            .setCustomerId(customerId)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleAddItem(
        state: CartState,
        productId: String,
        name: String,
        quantity: Int,
        unitPriceCents: Int
    ): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }
        if (quantity <= 0) {
            throw CommandValidationException.invalidArgument("Quantity must be positive")
        }
        if (state.items.any { it.productId == productId }) {
            throw CommandValidationException.failedPrecondition("Item already in cart, use UpdateQuantity")
        }

        val item = LineItem.newBuilder()
            .setProductId(productId)
            .setName(name)
            .setQuantity(quantity)
            .setUnitPriceCents(unitPriceCents)
            .build()

        val itemTotal = quantity * unitPriceCents
        val newSubtotal = state.subtotalCents + itemTotal

        logger.info("adding_item product_id={} quantity={} new_subtotal={}", productId, quantity, newSubtotal)

        val event = ItemAdded.newBuilder()
            .setItem(item)
            .setNewSubtotalCents(newSubtotal)
            .build()

        return createEventBook(event)
    }

    private fun handleUpdateQuantity(state: CartState, productId: String, quantity: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }
        if (quantity <= 0) {
            throw CommandValidationException.invalidArgument("Quantity must be positive")
        }

        val existingItem = state.items.find { it.productId == productId }
            ?: throw CommandValidationException.failedPrecondition("Item not in cart")

        val oldItemTotal = existingItem.quantity * existingItem.unitPriceCents
        val newItemTotal = quantity * existingItem.unitPriceCents
        val newSubtotal = state.subtotalCents - oldItemTotal + newItemTotal

        logger.info("updating_quantity product_id={} old_qty={} new_qty={}", productId, existingItem.quantity, quantity)

        val event = QuantityUpdated.newBuilder()
            .setProductId(productId)
            .setOldQuantity(existingItem.quantity)
            .setNewQuantity(quantity)
            .setNewSubtotalCents(newSubtotal)
            .build()

        return createEventBook(event)
    }

    private fun handleRemoveItem(state: CartState, productId: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }

        val existingItem = state.items.find { it.productId == productId }
            ?: throw CommandValidationException.failedPrecondition("Item not in cart")

        val itemTotal = existingItem.quantity * existingItem.unitPriceCents
        val newSubtotal = state.subtotalCents - itemTotal

        logger.info("removing_item product_id={} new_subtotal={}", productId, newSubtotal)

        val event = ItemRemoved.newBuilder()
            .setProductId(productId)
            .setNewSubtotalCents(newSubtotal)
            .build()

        return createEventBook(event)
    }

    private fun handleApplyCoupon(state: CartState, couponCode: String, discountCents: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }
        if (couponCode.isEmpty()) {
            throw CommandValidationException.invalidArgument("Coupon code is required")
        }
        if (state.couponCode.isNotEmpty()) {
            throw CommandValidationException.failedPrecondition("Coupon already applied")
        }

        logger.info("applying_coupon code={} discount_cents={}", couponCode, discountCents)

        val event = CouponApplied.newBuilder()
            .setCouponCode(couponCode)
            .setDiscountCents(discountCents)
            .build()

        return createEventBook(event)
    }

    private fun handleClearCart(state: CartState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }

        logger.info("clearing_cart customer_id={}", state.customerId)

        val event = CartCleared.newBuilder()
            .setClearedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleCheckout(state: CartState, loyaltyPointsToUse: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart does not exist")
        }
        if (state.isCheckedOut()) {
            throw CommandValidationException.failedPrecondition("Cart already checked out")
        }
        if (state.items.isEmpty()) {
            throw CommandValidationException.failedPrecondition("Cart is empty")
        }

        val totalCents = state.subtotalCents - state.discountCents

        logger.info("checkout_requested customer_id={} total_cents={} loyalty_points={}",
            state.customerId, totalCents, loyaltyPointsToUse)

        val event = CartCheckoutRequested.newBuilder()
            .setCustomerId(state.customerId)
            .addAllItems(state.items)
            .setSubtotalCents(state.subtotalCents)
            .setDiscountCents(state.discountCents)
            .setLoyaltyPointsToUse(loyaltyPointsToUse)
            .setRequestedAt(nowTimestamp())
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
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50502

    val service = CartService()
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
