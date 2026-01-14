package dev.angzarr.examples.saga

import dev.angzarr.CommandBook
import dev.angzarr.CommandPage
import dev.angzarr.Cover
import dev.angzarr.EventBook
import com.google.protobuf.Any
import examples.Domains.AddLoyaltyPoints
import examples.Domains.TransactionCompleted
import org.slf4j.LoggerFactory

/**
 * Interface for loyalty saga business logic.
 */
interface LoyaltySagaLogic {
    /**
     * Process events and generate commands to award loyalty points.
     *
     * @param eventBook the event book to process
     * @return list of command books to execute
     */
    fun processEvents(eventBook: EventBook): List<CommandBook>
}

/**
 * Default implementation of loyalty saga logic.
 */
class DefaultLoyaltySagaLogic : LoyaltySagaLogic {

    private val logger = LoggerFactory.getLogger(DefaultLoyaltySagaLogic::class.java)

    override fun processEvents(eventBook: EventBook): List<CommandBook> {
        val commands = mutableListOf<CommandBook>()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue

            if (event.`is`(TransactionCompleted::class.java)) {
                val cmd = handleTransactionCompleted(event, eventBook)
                if (cmd != null) {
                    commands.add(cmd)
                }
            }
        }

        return commands
    }

    private fun handleTransactionCompleted(event: Any, eventBook: EventBook): CommandBook? {
        val e = event.unpack(TransactionCompleted::class.java)
        val points = e.loyaltyPointsEarned

        if (points <= 0) {
            return null
        }

        val rootId = eventBook.cover?.root?.value?.toByteArray()
            ?.joinToString("") { b -> "%02x".format(b) } ?: ""

        logger.info("generating_add_loyalty_points transaction={} points={}", rootId.take(16), points)

        val cmd = AddLoyaltyPoints.newBuilder()
            .setPoints(points)
            .setReason("transaction:$rootId")
            .build()

        return CommandBook.newBuilder()
            .setCover(
                Cover.newBuilder()
                    .setDomain("customer")
                    .setRoot(eventBook.cover?.root)
            )
            .addPages(
                CommandPage.newBuilder()
                    .setCommand(Any.pack(cmd))
            )
            .build()
    }
}
