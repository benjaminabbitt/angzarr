package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import com.google.protobuf.Any
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.LoyaltyPointsRedeemed

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
 * Default implementation of customer log projector logic.
 */
class DefaultCustomerLogProjectorLogic : LogProjectorLogic {

    override fun processEvents(eventBook: EventBook): List<LogEntry> {
        val domain = eventBook.cover?.domain ?: "customer"
        val rootId = eventBook.cover?.root?.value?.toByteArray()
            ?.joinToString("") { "%02x".format(it) } ?: ""

        return eventBook.pagesList.mapNotNull { page ->
            val event = page.event ?: return@mapNotNull null
            processEvent(event, domain, rootId, page.num)
        }
    }

    private fun processEvent(event: Any, domain: String, rootId: String, sequence: Int): LogEntry {
        return when {
            event.`is`(CustomerCreated::class.java) -> {
                val e = event.unpack(CustomerCreated::class.java)
                LogEntry(
                    eventType = "CustomerCreated",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "name" to e.name,
                        "email" to e.email
                    )
                )
            }
            event.`is`(LoyaltyPointsAdded::class.java) -> {
                val e = event.unpack(LoyaltyPointsAdded::class.java)
                LogEntry(
                    eventType = "LoyaltyPointsAdded",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "points" to e.points.toString(),
                        "balance" to e.newBalance.toString(),
                        "reason" to e.reason
                    )
                )
            }
            event.`is`(LoyaltyPointsRedeemed::class.java) -> {
                val e = event.unpack(LoyaltyPointsRedeemed::class.java)
                LogEntry(
                    eventType = "LoyaltyPointsRedeemed",
                    domain = domain,
                    rootId = rootId,
                    sequence = sequence,
                    fields = mapOf(
                        "points" to e.points.toString(),
                        "balance" to e.newBalance.toString(),
                        "type" to e.redemptionType
                    )
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
