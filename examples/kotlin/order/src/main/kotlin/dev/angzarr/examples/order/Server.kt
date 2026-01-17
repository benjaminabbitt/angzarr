package dev.angzarr.examples.order

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

private val logger = LoggerFactory.getLogger("OrderServer")
private const val DOMAIN = "order"

class OrderService : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

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
                cmd.`is`(CreateOrder::class.java) -> {
                    val c = cmd.unpack(CreateOrder::class.java)
                    handleCreateOrder(state, c.customerId, c.itemsList, c.subtotalCents, c.discountCents)
                }
                cmd.`is`(ApplyLoyaltyDiscount::class.java) -> {
                    val c = cmd.unpack(ApplyLoyaltyDiscount::class.java)
                    handleApplyLoyaltyDiscount(state, c.pointsUsed, c.discountCents)
                }
                cmd.`is`(SubmitPayment::class.java) -> {
                    val c = cmd.unpack(SubmitPayment::class.java)
                    handleSubmitPayment(state, c.paymentMethod, c.amountCents)
                }
                cmd.`is`(ConfirmPayment::class.java) -> {
                    handleConfirmPayment(state)
                }
                cmd.`is`(CancelOrder::class.java) -> {
                    val c = cmd.unpack(CancelOrder::class.java)
                    handleCancelOrder(state, c.reason)
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

    private fun rebuildState(eventBook: EventBook?): OrderState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return OrderState.empty()
        }

        var state = OrderState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: OrderState, event: Any): OrderState {
        return when {
            event.`is`(OrderCreated::class.java) -> {
                val e = event.unpack(OrderCreated::class.java)
                state.copy(
                    customerId = e.customerId,
                    items = e.itemsList,
                    subtotalCents = e.subtotalCents,
                    discountCents = e.discountCents,
                    status = "pending_payment"
                )
            }
            event.`is`(LoyaltyDiscountApplied::class.java) -> {
                val e = event.unpack(LoyaltyDiscountApplied::class.java)
                state.copy(
                    loyaltyPointsUsed = e.pointsUsed,
                    discountCents = state.discountCents + e.discountCents
                )
            }
            event.`is`(PaymentSubmitted::class.java) -> {
                val e = event.unpack(PaymentSubmitted::class.java)
                state.copy(
                    paymentMethod = e.paymentMethod,
                    finalTotalCents = e.amountCents,
                    status = "paid"
                )
            }
            event.`is`(OrderCompleted::class.java) -> {
                state.copy(status = "completed")
            }
            event.`is`(OrderCancelled::class.java) -> {
                state.copy(status = "cancelled")
            }
            else -> state
        }
    }

    private fun handleCreateOrder(
        state: OrderState,
        customerId: String,
        items: List<LineItem>,
        subtotalCents: Int,
        discountCents: Int
    ): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Order already exists")
        }
        if (customerId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Customer ID is required")
        }
        if (items.isEmpty()) {
            throw CommandValidationException.invalidArgument("Order must have at least one item")
        }

        logger.info("creating_order customer_id={} subtotal={} discount={}",
            customerId, subtotalCents, discountCents)

        val event = OrderCreated.newBuilder()
            .setCustomerId(customerId)
            .addAllItems(items)
            .setSubtotalCents(subtotalCents)
            .setDiscountCents(discountCents)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleApplyLoyaltyDiscount(state: OrderState, pointsUsed: Int, discountCents: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Order does not exist")
        }
        if (!state.isPendingPayment()) {
            throw CommandValidationException.failedPrecondition("Order is not pending payment")
        }
        if (pointsUsed <= 0) {
            throw CommandValidationException.invalidArgument("Points must be positive")
        }

        logger.info("applying_loyalty_discount points={} discount_cents={}", pointsUsed, discountCents)

        val event = LoyaltyDiscountApplied.newBuilder()
            .setPointsUsed(pointsUsed)
            .setDiscountCents(discountCents)
            .build()

        return createEventBook(event)
    }

    private fun handleSubmitPayment(state: OrderState, paymentMethod: String, amountCents: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Order does not exist")
        }
        if (!state.isPendingPayment()) {
            throw CommandValidationException.failedPrecondition("Order is not pending payment")
        }
        if (paymentMethod.isEmpty()) {
            throw CommandValidationException.invalidArgument("Payment method is required")
        }

        val expectedAmount = state.subtotalCents - state.discountCents
        if (amountCents != expectedAmount) {
            throw CommandValidationException.invalidArgument(
                "Payment amount $amountCents does not match expected $expectedAmount"
            )
        }

        logger.info("submitting_payment method={} amount={}", paymentMethod, amountCents)

        val event = PaymentSubmitted.newBuilder()
            .setPaymentMethod(paymentMethod)
            .setAmountCents(amountCents)
            .setSubmittedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleConfirmPayment(state: OrderState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Order does not exist")
        }
        if (!state.isPaid()) {
            throw CommandValidationException.failedPrecondition("Order payment not submitted")
        }

        logger.info("confirming_payment customer_id={} total={}", state.customerId, state.finalTotalCents)

        val event = OrderCompleted.newBuilder()
            .setCompletedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleCancelOrder(state: OrderState, reason: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Order does not exist")
        }
        if (state.isCancelled()) {
            throw CommandValidationException.failedPrecondition("Order already cancelled")
        }
        if (state.isCompleted()) {
            throw CommandValidationException.failedPrecondition("Cannot cancel completed order")
        }

        logger.info("cancelling_order customer_id={} reason={}", state.customerId, reason)

        val event = OrderCancelled.newBuilder()
            .setReason(reason)
            .setLoyaltyPointsUsed(state.loyaltyPointsUsed)
            .setCancelledAt(nowTimestamp())
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
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50503

    val service = OrderService()
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
