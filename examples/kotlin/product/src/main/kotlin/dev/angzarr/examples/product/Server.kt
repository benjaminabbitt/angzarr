package dev.angzarr.examples.product

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

private val logger = LoggerFactory.getLogger("ProductServer")
private const val DOMAIN = "product"

class ProductService : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

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
                cmd.`is`(CreateProduct::class.java) -> {
                    val c = cmd.unpack(CreateProduct::class.java)
                    handleCreateProduct(state, c.sku, c.name, c.description, c.priceCents)
                }
                cmd.`is`(UpdateProduct::class.java) -> {
                    val c = cmd.unpack(UpdateProduct::class.java)
                    handleUpdateProduct(state, c.name, c.description)
                }
                cmd.`is`(SetPrice::class.java) -> {
                    val c = cmd.unpack(SetPrice::class.java)
                    handleSetPrice(state, c.priceCents)
                }
                cmd.`is`(Discontinue::class.java) -> {
                    handleDiscontinue(state)
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

    private fun rebuildState(eventBook: EventBook?): ProductState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return ProductState.empty()
        }

        var state = ProductState.empty()

        eventBook.snapshot?.state?.let { snapAny ->
            if (snapAny.`is`(examples.Domains.ProductState::class.java)) {
                val snapState = snapAny.unpack(examples.Domains.ProductState::class.java)
                state = ProductState(
                    sku = snapState.sku,
                    name = snapState.name,
                    description = snapState.description,
                    priceCents = snapState.priceCents,
                    status = snapState.status
                )
            }
        }

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: ProductState, event: Any): ProductState {
        return when {
            event.`is`(ProductCreated::class.java) -> {
                val e = event.unpack(ProductCreated::class.java)
                state.copy(
                    sku = e.sku,
                    name = e.name,
                    description = e.description,
                    priceCents = e.priceCents,
                    status = "active"
                )
            }
            event.`is`(ProductUpdated::class.java) -> {
                val e = event.unpack(ProductUpdated::class.java)
                state.copy(name = e.name, description = e.description)
            }
            event.`is`(PriceSet::class.java) -> {
                val e = event.unpack(PriceSet::class.java)
                state.copy(priceCents = e.newPriceCents)
            }
            event.`is`(ProductDiscontinued::class.java) -> {
                state.copy(status = "discontinued")
            }
            else -> state
        }
    }

    private fun handleCreateProduct(
        state: ProductState,
        sku: String,
        name: String,
        description: String,
        priceCents: Int
    ): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Product already exists")
        }
        if (sku.isEmpty()) {
            throw CommandValidationException.invalidArgument("SKU is required")
        }
        if (name.isEmpty()) {
            throw CommandValidationException.invalidArgument("Product name is required")
        }
        if (priceCents <= 0) {
            throw CommandValidationException.invalidArgument("Price must be positive")
        }

        logger.info("creating_product sku={} name={} price_cents={}", sku, name, priceCents)

        val event = ProductCreated.newBuilder()
            .setSku(sku)
            .setName(name)
            .setDescription(description)
            .setPriceCents(priceCents)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    private fun handleUpdateProduct(state: ProductState, name: String, description: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist")
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Cannot update discontinued product")
        }
        if (name.isEmpty()) {
            throw CommandValidationException.invalidArgument("Product name is required")
        }

        logger.info("updating_product sku={} name={}", state.sku, name)

        val event = ProductUpdated.newBuilder()
            .setName(name)
            .setDescription(description)
            .build()

        return createEventBook(event)
    }

    private fun handleSetPrice(state: ProductState, priceCents: Int): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist")
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Cannot set price on discontinued product")
        }
        if (priceCents <= 0) {
            throw CommandValidationException.invalidArgument("Price must be positive")
        }

        logger.info("setting_price sku={} old_price={} new_price={}", state.sku, state.priceCents, priceCents)

        val event = PriceSet.newBuilder()
            .setOldPriceCents(state.priceCents)
            .setNewPriceCents(priceCents)
            .build()

        return createEventBook(event)
    }

    private fun handleDiscontinue(state: ProductState): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist")
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Product already discontinued")
        }

        logger.info("discontinuing_product sku={}", state.sku)

        val event = ProductDiscontinued.newBuilder()
            .setDiscontinuedAt(nowTimestamp())
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
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50501

    val service = ProductService()
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
