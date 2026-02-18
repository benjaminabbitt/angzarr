import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    'intro',
    'architecture',
    'getting-started',
    {
      type: 'category',
      label: 'Concepts',
      items: ['concepts/cqrs-event-sourcing'],
    },
    {
      type: 'category',
      label: 'Components',
      items: [
        'components/aggregate',
        'components/saga',
        'components/projector',
        'components/process-manager',
      ],
    },
    {
      type: 'category',
      label: 'Tooling',
      items: [
        'tooling/just',
        'tooling/cucumber',
        'tooling/testcontainers',
        'tooling/just-overlays',
        'tooling/scm',
        'tooling/claude',
        {
          type: 'category',
          label: 'Databases',
          items: [
            'tooling/databases/postgres',
            'tooling/databases/redis',
            'tooling/databases/sqlite',
            'tooling/databases/bigtable',
            'tooling/databases/dynamo',
            'tooling/databases/immudb',
          ],
        },
        {
          type: 'category',
          label: 'Message Buses',
          items: [
            'tooling/buses/amqp',
            'tooling/buses/kafka',
            'tooling/buses/pubsub',
            'tooling/buses/sns-sqs',
            'tooling/buses/nats',
          ],
        },
      ],
    },
    {
      type: 'category',
      label: 'Operations',
      items: [
        'operations/testing',
        'operations/observability',
        'operations/infrastructure',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: [
        'reference/patterns',
        'reference/port-conventions',
      ],
    },
    {
      type: 'category',
      label: 'Examples',
      items: [
        'examples/why-poker',
        'examples/aggregates',
        'examples/sagas',
        'examples/language-notes',
      ],
    },
  ],
};

export default sidebars;
