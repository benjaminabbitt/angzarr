#pragma once

#include <string>
#include "angzarr.pb.h"
#include "domains.pb.h"

namespace product {

enum class ProductStatus { Uninitialized, Active, Discontinued };

struct ProductState {
    std::string sku;
    std::string name;
    std::string description;
    int32_t price_cents = 0;
    ProductStatus status = ProductStatus::Uninitialized;

    bool exists() const { return status != ProductStatus::Uninitialized; }
    bool active() const { return status == ProductStatus::Active; }
};

class ProductLogic {
public:
    static ProductState rebuild_state(const angzarr::EventBook* event_book);

    static examples::ProductCreated handle_create_product(
        const ProductState& state, const std::string& sku, const std::string& name,
        const std::string& description, int32_t price_cents);

    static examples::ProductUpdated handle_update_product(
        const ProductState& state, const std::string& name, const std::string& description);

    static examples::PriceSet handle_set_price(const ProductState& state, int32_t price_cents);

    static examples::ProductDiscontinued handle_discontinue(
        const ProductState& state, const std::string& reason);

private:
    static ProductState apply_event(ProductState state, const google::protobuf::Any& event);
};

}  // namespace product
