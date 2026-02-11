# angzarr-client

Python client library for Angzarr CQRS/ES framework.

## Installation

```bash
pip install angzarr-client
```

## Usage

```python
from angzarr_client import DomainClient

# Connect to a domain's aggregate coordinator
client = DomainClient("localhost:1310")

# Build and execute a command
response = client.command("order", root_id) \
    .with_command("CreateOrder", create_order_msg) \
    .execute()

# Query events
events = client.query("order", root_id) \
    .get_event_book()
```

## License

AGPL-3.0 - See [LICENSE](LICENSE) for details.
