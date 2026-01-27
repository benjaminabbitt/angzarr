"""Handler exceptions and error message constants."""


class errmsg:
    """Error message constants for fulfillment domain."""

    SHIPMENT_EXISTS = "Shipment already exists"
    SHIPMENT_NOT_FOUND = "Shipment does not exist"
    NOT_PENDING = "Shipment is not pending"
    NOT_PICKED = "Shipment is not picked"
    NOT_PACKED = "Shipment is not packed"
    NOT_SHIPPED = "Shipment is not shipped"
    ALREADY_DELIVERED = "Shipment is already delivered"
    UNKNOWN_COMMAND = "Unknown command type"
    NO_COMMAND_PAGES = "CommandBook has no pages"
    ORDER_ID_REQUIRED = "Order ID is required"
    PICKER_ID_REQUIRED = "Picker ID is required"
    PACKER_ID_REQUIRED = "Packer ID is required"
    CARRIER_REQUIRED = "Carrier is required"
    TRACKING_NUMBER_REQUIRED = "Tracking number is required"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""
