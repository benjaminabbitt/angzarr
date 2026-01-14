package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import com.google.protobuf.Any
import examples.Domains.DiscountApplied
import examples.Domains.TransactionCancelled
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated

/**
 * Result of processing an event for logging.
 */
data class LogEntry(
    val eventType: String,
    val domain: String,
    val rootId: String,
    val sequence: Int,
    val fields: Map<String, String>
)

/**
 * Interface for log projector business logic.
 */
interface LogProjectorLogic {
    /**
     * Process an event book and return log entries for each event.
     *
     * @param eventBook the event book to process
     * @return list of log entries
     */
    fun processEvents(eventBook: EventBook): List<LogEntry>
}

/**
 * Default implementation of transaction log projector logic.
 */
class DefaultTransactionLogProjectorLogic : LogProjectorLogic {

    override fun processEvents(eventBook: EventBook): List<LogEntry> {
        val domain = eventBook.cover?.domain ?: "transaction"
        val rootId = eventBook.cover?.root?.value?.toByteArray()
            ?.joinToString("") { "%02x".format(it) } ?: ""

        return eventBook.pagesList.mapNotNull { page ->
            val event = page.event ?: return@mapNotNull null
            processEvent(event, domain, rootId, page.num)
        }
    }

    private fun processEvent(event: Any, domain: String, rootId: String, sequence: Int): LogEntry {
        return when {
            event.`is`(TransactionCreated::class.java) -> {
                val e = event.unpack(TransactionCreated::class.java)
                LogEntry(
                    eventType = "TransactionCreated",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "customer" to e.customerId,
                        "items" to e.itemsCount.toString(),
                        "subtotal" to e.subtotalCents.toString()
                    )
                )
            }
            event.`is`(DiscountApplied::class.java) -> {
                val e = event.unpack(DiscountApplied::class.java)
                LogEntry(
                    eventType = "DiscountApplied",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "discount_type" to e.discountType,
                        "value" to e.value.toString(),
                        "cents" to e.discountCents.toString()
                    )
                )
            }
            event.`is`(TransactionCompleted::class.java) -> {
                val e = event.unpack(TransactionCompleted::class.java)
                LogEntry(
                    eventType = "TransactionCompleted",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "total" to e.finalTotalCents.toString(),
                        "payment" to e.paymentMethod,
                        "points" to e.loyaltyPointsEarned.toString()
                    )
                )
            }
            event.`is`(TransactionCancelled::class.java) -> {
                val e = event.unpack(TransactionCancelled::class.java)
                LogEntry(
                    eventType = "TransactionCancelled",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf("reason" to e.reason)
                )
            }
            else -> LogEntry(
                eventType = "unknown",
                domain = domain,
                rootId = rootId,
                sequence = sequence,
                fields = emptyMap()
            )
        }
    }
}
