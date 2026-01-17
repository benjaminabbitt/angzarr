#include "logging.hpp"
#include "angzarr.grpc.pb.h"
#include "domains.pb.h"
#include <grpcpp/grpcpp.h>
#include <sstream>
#include <iomanip>
#include <ctime>

namespace projector_receipt {

class ReceiptProjectorService final : public angzarr::ProjectorCoordinator::Service {
public:
    grpc::Status Project(grpc::ServerContext* context,
                         const angzarr::EventBook* request,
                         angzarr::Projection* response) override {
        std::ostringstream receipt;
        receipt << std::fixed << std::setprecision(2);

        std::string customer_id;
        std::string order_id = request->cover().entity_id();
        int32_t subtotal_cents = 0;
        int32_t discount_cents = 0;
        int32_t total_cents = 0;
        std::string status = "pending";

        for (const auto& page : request->pages()) {
            if (!page.has_event()) continue;

            const auto& type_url = page.event().type_url();

            if (type_url.find("OrderCreated") != std::string::npos) {
                examples::OrderCreated event;
                page.event().UnpackTo(&event);
                customer_id = event.customer_id();
                subtotal_cents = event.subtotal_cents();
                discount_cents = event.discount_cents();
                total_cents = event.total_cents();

                receipt << "========================================\n";
                receipt << "           ORDER RECEIPT\n";
                receipt << "========================================\n";
                receipt << "Order ID: " << order_id << "\n";
                receipt << "Customer: " << customer_id << "\n";
                receipt << "----------------------------------------\n";
                receipt << "Items:\n";

                for (const auto& item : event.items()) {
                    double price = item.price_cents() / 100.0;
                    double line_total = (item.price_cents() * item.quantity()) / 100.0;
                    receipt << "  " << item.product_id() << "\n";
                    receipt << "    " << item.quantity() << " x $" << price << " = $" << line_total << "\n";
                }

                receipt << "----------------------------------------\n";
                receipt << "Subtotal: $" << (subtotal_cents / 100.0) << "\n";
                if (discount_cents > 0) {
                    receipt << "Discount: -$" << (discount_cents / 100.0) << "\n";
                }
                receipt << "Total: $" << (total_cents / 100.0) << "\n";
            } else if (type_url.find("LoyaltyDiscountApplied") != std::string::npos) {
                examples::LoyaltyDiscountApplied event;
                page.event().UnpackTo(&event);
                receipt << "Loyalty Points Used: " << event.points_used() << "\n";
            } else if (type_url.find("PaymentSubmitted") != std::string::npos) {
                examples::PaymentSubmitted event;
                page.event().UnpackTo(&event);
                status = "payment_pending";
                receipt << "Payment: " << event.payment_method() << "\n";
            } else if (type_url.find("OrderCompleted") != std::string::npos) {
                examples::OrderCompleted event;
                page.event().UnpackTo(&event);
                status = "completed";
                receipt << "Status: COMPLETED\n";
                if (event.loyalty_points_earned() > 0) {
                    receipt << "Points Earned: " << event.loyalty_points_earned() << "\n";
                }
            } else if (type_url.find("OrderCancelled") != std::string::npos) {
                examples::OrderCancelled event;
                page.event().UnpackTo(&event);
                status = "cancelled";
                receipt << "Status: CANCELLED\n";
                receipt << "Reason: " << event.reason() << "\n";
            }
        }

        receipt << "========================================\n";
        receipt << "        Thank you for your order!\n";
        receipt << "========================================\n";

        response->set_data(receipt.str());

        angzarr::log_info("projector-receipt", "receipt_projected",
            {{"order_id", order_id}, {"status", status}});

        return grpc::Status::OK;
    }
};

std::unique_ptr<angzarr::ProjectorCoordinator::Service> create_projector_receipt_service() {
    return std::make_unique<ReceiptProjectorService>();
}

}  // namespace projector_receipt
