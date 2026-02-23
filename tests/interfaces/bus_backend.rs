//! Bus backend factory for interface tests.
//!
//! Provides a unified interface to create bus backends based on environment configuration.

use std::env;
use std::sync::Arc;

use angzarr::bus::EventBus;
use angzarr::dlq::{AngzarrDeadLetter, DeadLetterPublisher};
use tokio::sync::mpsc;

#[cfg(any(
    feature = "amqp",
    feature = "kafka",
    feature = "nats",
    feature = "pubsub",
    feature = "sns-sqs"
))]
use std::time::Duration;

#[cfg(feature = "channel")]
use angzarr::bus::channel::ChannelEventBus;

#[cfg(feature = "channel")]
use angzarr::dlq::ChannelDeadLetterPublisher;

#[cfg(feature = "amqp")]
use angzarr::bus::amqp::AmqpEventBus;

#[cfg(feature = "amqp")]
use angzarr::dlq::AmqpDeadLetterPublisher;

#[cfg(feature = "kafka")]
use angzarr::bus::kafka::KafkaEventBus;

#[cfg(feature = "kafka")]
use angzarr::dlq::KafkaDeadLetterPublisher;

#[cfg(feature = "nats")]
use angzarr::bus::nats::NatsEventBus;

#[cfg(feature = "pubsub")]
use angzarr::bus::pubsub::PubSubEventBus;

#[cfg(feature = "pubsub")]
use angzarr::dlq::PubSubDeadLetterPublisher;

#[cfg(feature = "sns-sqs")]
use angzarr::bus::sns_sqs::SnsSqsEventBus;

#[cfg(feature = "sns-sqs")]
use angzarr::dlq::SnsSqsDeadLetterPublisher;

#[cfg(any(
    feature = "amqp",
    feature = "kafka",
    feature = "nats",
    feature = "pubsub",
    feature = "sns-sqs"
))]
use testcontainers::{
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
    GenericImage, ImageExt,
};

/// Bus backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusBackend {
    Channel,
    Amqp,
    Kafka,
    Nats,
    PubSub,
    SnsSqs,
}

impl BusBackend {
    pub fn from_env() -> Self {
        match env::var("BUS_BACKEND")
            .unwrap_or_else(|_| "channel".to_string())
            .to_lowercase()
            .as_str()
        {
            "amqp" => BusBackend::Amqp,
            "kafka" => BusBackend::Kafka,
            "nats" => BusBackend::Nats,
            "pubsub" => BusBackend::PubSub,
            "sns-sqs" | "sns_sqs" => BusBackend::SnsSqs,
            _ => BusBackend::Channel,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            BusBackend::Channel => "channel",
            BusBackend::Amqp => "amqp",
            BusBackend::Kafka => "kafka",
            BusBackend::Nats => "nats",
            BusBackend::PubSub => "pubsub",
            BusBackend::SnsSqs => "sns-sqs",
        }
    }
}

/// Container handles to keep containers alive during tests.
#[allow(dead_code)]
#[derive(Debug)]
pub enum BusContainerHandle {
    None,
    #[cfg(feature = "amqp")]
    Amqp(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "kafka")]
    Kafka(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "nats")]
    Nats(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "pubsub")]
    PubSub(testcontainers::ContainerAsync<GenericImage>),
    #[cfg(feature = "sns-sqs")]
    SnsSqs(testcontainers::ContainerAsync<GenericImage>),
}

/// Subscriber factory function type.
pub type SubscriberFactory = Box<
    dyn Fn(&str, Option<&str>) -> futures::future::BoxFuture<'static, Arc<dyn EventBus>>
        + Send
        + Sync,
>;

/// Holds the bus implementations for a backend.
pub struct BusContext {
    /// Publisher for sending events.
    pub publisher: Arc<dyn EventBus>,
    /// Factory for creating subscribers.
    /// Arguments: (name, domain_filter)
    subscriber_factory: SubscriberFactory,
    /// Container handle to keep container alive.
    #[allow(dead_code)]
    container: BusContainerHandle,
}

impl std::fmt::Debug for BusContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BusContext")
            .field("publisher", &"<dyn EventBus>")
            .field("subscriber_factory", &"<fn>")
            .field("container", &self.container)
            .finish()
    }
}

impl BusContext {
    /// Create a subscriber for a domain.
    pub async fn create_subscriber(&self, name: &str, domain: Option<&str>) -> Arc<dyn EventBus> {
        (self.subscriber_factory)(name, domain).await
    }

    /// Create a bus context for the configured backend.
    pub async fn new(backend: BusBackend) -> Self {
        match backend {
            BusBackend::Channel => Self::create_channel().await,
            BusBackend::Amqp => Self::create_amqp().await,
            BusBackend::Kafka => Self::create_kafka().await,
            BusBackend::Nats => Self::create_nats().await,
            BusBackend::PubSub => Self::create_pubsub().await,
            BusBackend::SnsSqs => Self::create_sns_sqs().await,
        }
    }

    #[cfg(feature = "channel")]
    async fn create_channel() -> Self {
        let bus = Arc::new(ChannelEventBus::publisher());
        let bus_clone = bus.clone();

        BusContext {
            publisher: bus,
            subscriber_factory: Box::new(move |_name, domain| {
                let bus = bus_clone.clone();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    match domain {
                        Some(d) => bus
                            .create_subscriber("", Some(&d))
                            .await
                            .expect("Failed to create subscriber"),
                        None => bus
                            .create_subscriber("", None)
                            .await
                            .expect("Failed to create subscriber"),
                    }
                })
            }),
            container: BusContainerHandle::None,
        }
    }

    #[cfg(not(feature = "channel"))]
    async fn create_channel() -> Self {
        panic!("Channel feature not enabled. Build with --features channel");
    }

    #[cfg(feature = "amqp")]
    async fn create_amqp() -> Self {
        let image = GenericImage::new("rabbitmq", "3-management")
            .with_exposed_port(5672.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Server startup complete"));

        let container = image
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start RabbitMQ container");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(5672)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let url = format!("amqp://guest:guest@{}:{}", host, host_port);

        let bus = AmqpEventBus::publisher(&url)
            .await
            .expect("Failed to create AMQP publisher");

        let url_clone = url.clone();

        BusContext {
            publisher: Arc::new(bus),
            subscriber_factory: Box::new(move |name, domain| {
                let url = url_clone.clone();
                let name = name.to_string();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    let subscriber = match domain {
                        Some(d) => AmqpEventBus::subscriber(&url, &name, &d)
                            .await
                            .expect("Failed to create AMQP subscriber"),
                        None => AmqpEventBus::subscriber_all(&url, &name)
                            .await
                            .expect("Failed to create AMQP subscriber"),
                    };
                    Arc::new(subscriber) as Arc<dyn EventBus>
                })
            }),
            container: BusContainerHandle::Amqp(container),
        }
    }

    #[cfg(not(feature = "amqp"))]
    async fn create_amqp() -> Self {
        panic!("AMQP feature not enabled. Build with --features amqp");
    }

    #[cfg(feature = "kafka")]
    async fn create_kafka() -> Self {
        use rand::Rng;

        // Generate unique ports to avoid conflicts
        let kafka_port: u16 = rand::rng().random_range(29000..30000);

        let image = GenericImage::new("redpandadata/redpanda", "v24.1.1")
            .with_exposed_port(kafka_port.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Successfully started Redpanda!"))
            .with_cmd(vec![
                "redpanda",
                "start",
                "--kafka-addr",
                &format!("0.0.0.0:{}", kafka_port),
                "--advertise-kafka-addr",
                &format!("localhost:{}", kafka_port),
                "--smp",
                "1",
                "--memory",
                "512M",
                "--overprovisioned",
                "--node-id",
                "0",
                "--check=false",
            ]);

        let container = image
            .with_mapped_port(kafka_port, kafka_port.tcp())
            .with_startup_timeout(Duration::from_secs(120))
            .start()
            .await
            .expect("Failed to start Redpanda container");

        tokio::time::sleep(Duration::from_secs(3)).await;

        let bootstrap_servers = format!("localhost:{}", kafka_port);

        let bus = KafkaEventBus::publisher(&bootstrap_servers, "test")
            .expect("Failed to create Kafka publisher");

        let bootstrap_clone = bootstrap_servers.clone();

        BusContext {
            publisher: Arc::new(bus),
            subscriber_factory: Box::new(move |name, domain| {
                let bootstrap = bootstrap_clone.clone();
                let name = name.to_string();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    let subscriber = match domain {
                        Some(d) => KafkaEventBus::subscriber(&bootstrap, "test", &name, vec![d])
                            .expect("Failed to create Kafka subscriber"),
                        None => KafkaEventBus::subscriber_all(&bootstrap, "test", &name)
                            .expect("Failed to create Kafka subscriber"),
                    };
                    Arc::new(subscriber) as Arc<dyn EventBus>
                })
            }),
            container: BusContainerHandle::Kafka(container),
        }
    }

    #[cfg(not(feature = "kafka"))]
    async fn create_kafka() -> Self {
        panic!("Kafka feature not enabled. Build with --features kafka");
    }

    #[cfg(feature = "nats")]
    async fn create_nats() -> Self {
        let image = GenericImage::new("nats", "2.10")
            .with_exposed_port(4222.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "Listening for client connections",
            ))
            .with_cmd(vec!["-js"]);

        let container = image
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start NATS container");

        let host_port = container
            .get_host_port_ipv4(4222)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let url = format!("nats://{}:{}", host, host_port);

        let client = async_nats::connect(&url)
            .await
            .expect("Failed to connect to NATS");

        let prefix = format!(
            "test_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
        );

        let bus = NatsEventBus::new(client.clone(), Some(&prefix))
            .await
            .expect("Failed to create NATS bus");

        let prefix_clone = prefix.clone();

        BusContext {
            publisher: Arc::new(bus),
            subscriber_factory: Box::new(move |name, domain| {
                let url = url.clone();
                let prefix = prefix_clone.clone();
                let name = name.to_string();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    let client = async_nats::connect(&url)
                        .await
                        .expect("Failed to connect to NATS");
                    let bus = NatsEventBus::new(client, Some(&prefix))
                        .await
                        .expect("Failed to create NATS bus");
                    let domain_ref = domain.as_deref();
                    bus.create_subscriber(&name, domain_ref)
                        .await
                        .expect("Failed to create subscriber")
                })
            }),
            container: BusContainerHandle::Nats(container),
        }
    }

    #[cfg(not(feature = "nats"))]
    async fn create_nats() -> Self {
        panic!("NATS feature not enabled. Build with --features nats");
    }

    #[cfg(feature = "pubsub")]
    async fn create_pubsub() -> Self {
        let image = GenericImage::new(
            "gcr.io/google.com/cloudsdktool/google-cloud-cli",
            "emulators",
        )
        .with_exposed_port(8085.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Server started"))
        .with_cmd(vec![
            "gcloud",
            "beta",
            "emulators",
            "pubsub",
            "start",
            "--host-port=0.0.0.0:8085",
        ]);

        let container = image
            .with_startup_timeout(Duration::from_secs(120))
            .start()
            .await
            .expect("Failed to start Pub/Sub emulator");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(8085)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let endpoint = format!("http://{}:{}", host, host_port);

        // Set emulator environment variable
        std::env::set_var("PUBSUB_EMULATOR_HOST", &format!("{}:{}", host, host_port));

        let bus = PubSubEventBus::new("test-project", "test")
            .await
            .expect("Failed to create Pub/Sub bus");

        let endpoint_clone = endpoint.clone();

        BusContext {
            publisher: Arc::new(bus),
            subscriber_factory: Box::new(move |name, domain| {
                let _endpoint = endpoint_clone.clone();
                let name = name.to_string();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    let bus = PubSubEventBus::new("test-project", "test")
                        .await
                        .expect("Failed to create Pub/Sub bus");
                    let domain_ref = domain.as_deref();
                    bus.create_subscriber(&name, domain_ref)
                        .await
                        .expect("Failed to create subscriber")
                })
            }),
            container: BusContainerHandle::PubSub(container),
        }
    }

    #[cfg(not(feature = "pubsub"))]
    async fn create_pubsub() -> Self {
        panic!("Pub/Sub feature not enabled. Build with --features pubsub");
    }

    #[cfg(feature = "sns-sqs")]
    async fn create_sns_sqs() -> Self {
        let image = GenericImage::new("localstack/localstack", "latest")
            .with_exposed_port(4566.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready."));

        let container = image
            .with_env_var("SERVICES", "sns,sqs")
            .with_env_var("AWS_DEFAULT_REGION", "us-east-1")
            .with_env_var("EAGER_SERVICE_LOADING", "1")
            .with_startup_timeout(Duration::from_secs(180))
            .start()
            .await
            .expect("Failed to start LocalStack container");

        tokio::time::sleep(Duration::from_secs(5)).await;

        let host_port = container
            .get_host_port_ipv4(4566)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let endpoint = format!("http://{}:{}", host, host_port);

        // Set AWS credentials for LocalStack
        std::env::set_var("AWS_ACCESS_KEY_ID", "test");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");

        let bus = SnsSqsEventBus::new(Some("us-east-1"), Some(&endpoint), "test")
            .await
            .expect("Failed to create SNS/SQS bus");

        let endpoint_clone = endpoint.clone();

        BusContext {
            publisher: Arc::new(bus),
            subscriber_factory: Box::new(move |name, domain| {
                let endpoint = endpoint_clone.clone();
                let name = name.to_string();
                let domain = domain.map(|s| s.to_string());
                Box::pin(async move {
                    let bus = SnsSqsEventBus::new(Some("us-east-1"), Some(&endpoint), "test")
                        .await
                        .expect("Failed to create SNS/SQS bus");
                    let domain_ref = domain.as_deref();
                    bus.create_subscriber(&name, domain_ref)
                        .await
                        .expect("Failed to create subscriber")
                })
            }),
            container: BusContainerHandle::SnsSqs(container),
        }
    }

    #[cfg(not(feature = "sns-sqs"))]
    async fn create_sns_sqs() -> Self {
        panic!("SNS/SQS feature not enabled. Build with --features sns-sqs");
    }
}

/// Holds the DLQ publisher for a backend.
pub struct DlqContext {
    /// Publisher for sending dead letters.
    pub publisher: Arc<dyn DeadLetterPublisher>,
    /// Receiver for channel backend (None for distributed backends).
    pub receiver: Option<mpsc::UnboundedReceiver<AngzarrDeadLetter>>,
    /// Container handle to keep container alive.
    #[allow(dead_code)]
    container: BusContainerHandle,
}

impl std::fmt::Debug for DlqContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DlqContext")
            .field("publisher", &"<dyn DeadLetterPublisher>")
            .field("receiver", &self.receiver.is_some())
            .field("container", &self.container)
            .finish()
    }
}

impl DlqContext {
    /// Create a DlqContext with a noop publisher (for testing).
    pub fn noop() -> Self {
        use angzarr::dlq::NoopDeadLetterPublisher;
        DlqContext {
            publisher: Arc::new(NoopDeadLetterPublisher),
            receiver: None,
            container: BusContainerHandle::None,
        }
    }
}

impl DlqContext {
    /// Create a DLQ context for the configured backend.
    pub async fn new(backend: BusBackend) -> Self {
        match backend {
            BusBackend::Channel => Self::create_channel().await,
            BusBackend::Amqp => Self::create_amqp().await,
            BusBackend::Kafka => Self::create_kafka().await,
            BusBackend::Nats => Self::create_nats().await,
            BusBackend::PubSub => Self::create_pubsub().await,
            BusBackend::SnsSqs => Self::create_sns_sqs().await,
        }
    }

    #[cfg(feature = "channel")]
    async fn create_channel() -> Self {
        let (publisher, receiver) = ChannelDeadLetterPublisher::new();
        DlqContext {
            publisher: Arc::new(publisher),
            receiver: Some(receiver),
            container: BusContainerHandle::None,
        }
    }

    #[cfg(not(feature = "channel"))]
    async fn create_channel() -> Self {
        panic!("Channel feature not enabled. Build with --features channel");
    }

    #[cfg(feature = "amqp")]
    async fn create_amqp() -> Self {
        let image = GenericImage::new("rabbitmq", "3-management")
            .with_exposed_port(5672.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Server startup complete"));

        let container = image
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start RabbitMQ container");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(5672)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let url = format!("amqp://guest:guest@{}:{}", host, host_port);

        let publisher = AmqpDeadLetterPublisher::new(&url)
            .await
            .expect("Failed to create AMQP DLQ publisher");

        DlqContext {
            publisher: Arc::new(publisher),
            receiver: None,
            container: BusContainerHandle::Amqp(container),
        }
    }

    #[cfg(not(feature = "amqp"))]
    async fn create_amqp() -> Self {
        panic!("AMQP feature not enabled. Build with --features amqp");
    }

    #[cfg(feature = "kafka")]
    async fn create_kafka() -> Self {
        use rand::Rng;

        let kafka_port: u16 = rand::rng().random_range(29000..30000);

        let image = GenericImage::new("redpandadata/redpanda", "v24.1.1")
            .with_exposed_port(kafka_port.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Successfully started Redpanda!"))
            .with_cmd(vec![
                "redpanda",
                "start",
                "--kafka-addr",
                &format!("0.0.0.0:{}", kafka_port),
                "--advertise-kafka-addr",
                &format!("localhost:{}", kafka_port),
                "--smp",
                "1",
                "--memory",
                "512M",
                "--overprovisioned",
                "--node-id",
                "0",
                "--check=false",
            ]);

        let container = image
            .with_mapped_port(kafka_port, kafka_port.tcp())
            .with_startup_timeout(Duration::from_secs(120))
            .start()
            .await
            .expect("Failed to start Redpanda container");

        tokio::time::sleep(Duration::from_secs(3)).await;

        let bootstrap_servers = format!("localhost:{}", kafka_port);

        let publisher = KafkaDeadLetterPublisher::new(&bootstrap_servers)
            .expect("Failed to create Kafka DLQ publisher");

        DlqContext {
            publisher: Arc::new(publisher),
            receiver: None,
            container: BusContainerHandle::Kafka(container),
        }
    }

    #[cfg(not(feature = "kafka"))]
    async fn create_kafka() -> Self {
        panic!("Kafka feature not enabled. Build with --features kafka");
    }

    #[cfg(feature = "nats")]
    async fn create_nats() -> Self {
        // NATS doesn't have a dedicated DLQ publisher in the codebase.
        // For now, use the noop publisher. This could be extended in the future.
        use angzarr::dlq::NoopDeadLetterPublisher;

        let image = GenericImage::new("nats", "2.10")
            .with_exposed_port(4222.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "Listening for client connections",
            ))
            .with_cmd(vec!["-js"]);

        let container = image
            .with_startup_timeout(Duration::from_secs(60))
            .start()
            .await
            .expect("Failed to start NATS container");

        DlqContext {
            publisher: Arc::new(NoopDeadLetterPublisher),
            receiver: None,
            container: BusContainerHandle::Nats(container),
        }
    }

    #[cfg(not(feature = "nats"))]
    async fn create_nats() -> Self {
        panic!("NATS feature not enabled. Build with --features nats");
    }

    #[cfg(feature = "pubsub")]
    async fn create_pubsub() -> Self {
        let image = GenericImage::new(
            "gcr.io/google.com/cloudsdktool/google-cloud-cli",
            "emulators",
        )
        .with_exposed_port(8085.tcp())
        .with_wait_for(WaitFor::message_on_stderr("Server started"))
        .with_cmd(vec![
            "gcloud",
            "beta",
            "emulators",
            "pubsub",
            "start",
            "--host-port=0.0.0.0:8085",
        ]);

        let container = image
            .with_startup_timeout(Duration::from_secs(120))
            .start()
            .await
            .expect("Failed to start Pub/Sub emulator");

        tokio::time::sleep(Duration::from_secs(2)).await;

        let host_port = container
            .get_host_port_ipv4(8085)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");

        // Set emulator environment variable
        std::env::set_var("PUBSUB_EMULATOR_HOST", &format!("{}:{}", host, host_port));

        let publisher = PubSubDeadLetterPublisher::new()
            .await
            .expect("Failed to create Pub/Sub DLQ publisher");

        DlqContext {
            publisher: Arc::new(publisher),
            receiver: None,
            container: BusContainerHandle::PubSub(container),
        }
    }

    #[cfg(not(feature = "pubsub"))]
    async fn create_pubsub() -> Self {
        panic!("Pub/Sub feature not enabled. Build with --features pubsub");
    }

    #[cfg(feature = "sns-sqs")]
    async fn create_sns_sqs() -> Self {
        let image = GenericImage::new("localstack/localstack", "latest")
            .with_exposed_port(4566.tcp())
            .with_wait_for(WaitFor::message_on_stdout("Ready."));

        let container = image
            .with_env_var("SERVICES", "sns,sqs")
            .with_env_var("AWS_DEFAULT_REGION", "us-east-1")
            .with_env_var("EAGER_SERVICE_LOADING", "1")
            .with_startup_timeout(Duration::from_secs(180))
            .start()
            .await
            .expect("Failed to start LocalStack container");

        tokio::time::sleep(Duration::from_secs(5)).await;

        let host_port = container
            .get_host_port_ipv4(4566)
            .await
            .expect("Failed to get port");

        let host = container.get_host().await.expect("Failed to get host");
        let endpoint = format!("http://{}:{}", host, host_port);

        // Set AWS credentials for LocalStack
        std::env::set_var("AWS_ACCESS_KEY_ID", "test");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test");

        let publisher = SnsSqsDeadLetterPublisher::new(Some("us-east-1"), Some(&endpoint))
            .await
            .expect("Failed to create SNS/SQS DLQ publisher");

        DlqContext {
            publisher: Arc::new(publisher),
            receiver: None,
            container: BusContainerHandle::SnsSqs(container),
        }
    }

    #[cfg(not(feature = "sns-sqs"))]
    async fn create_sns_sqs() -> Self {
        panic!("SNS/SQS feature not enabled. Build with --features sns-sqs");
    }
}
