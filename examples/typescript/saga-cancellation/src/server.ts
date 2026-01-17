/**
 * Cancellation Saga - TypeScript Implementation
 *
 * Listens for OrderCancelled events and performs compensation.
 */

import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import protobuf from 'protobufjs';
import pino from 'pino';
import path from 'path';
import { fileURLToPath } from 'url';
import { HealthImplementation } from 'grpc-health-check';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROTO_PATH = path.resolve(__dirname, '../../../../proto');

const logger = (pino as unknown as (opts: object) => pino.Logger)({ level: 'info' });
const SAGA_NAME = 'cancellation';
const SOURCE_DOMAIN = 'order';

const packageDefinition = protoLoader.loadSync(
  [path.join(PROTO_PATH, 'angzarr/angzarr.proto')],
  {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
    includeDirs: [PROTO_PATH],
  }
);

const grpcProto = grpc.loadPackageDefinition(packageDefinition) as any;

let root: protobuf.Root;
let OrderCancelled: protobuf.Type;
let ReleaseReservation: protobuf.Type;
let AddLoyaltyPoints: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  OrderCancelled = root.lookupType('examples.OrderCancelled');
  ReleaseReservation = root.lookupType('examples.ReleaseReservation');
  AddLoyaltyPoints = root.lookupType('examples.AddLoyaltyPoints');
}

function encodeToAny(messageType: protobuf.Type, message: any, typeName: string): any {
  const encoded = messageType.encode(messageType.create(message)).finish();
  return {
    type_url: `type.examples/${typeName}`,
    value: Buffer.from(encoded),
  };
}

function processEvents(eventBook: any): any[] {
  if (!eventBook?.pages?.length) {
    return [];
  }

  const commands: any[] = [];

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    if (!typeUrl.endsWith('OrderCancelled')) {
      continue;
    }

    try {
      const event = OrderCancelled.decode(page.event.value) as any;

      let orderId = '';
      if (eventBook.cover?.root?.value) {
        const rootBytes = eventBook.cover.root.value;
        orderId = Buffer.from(rootBytes).toString('hex');
      }

      if (!orderId) continue;

      logger.info({ orderId }, 'processing order cancellation');

      const releaseCmd = { orderId };
      const releaseAny = encodeToAny(ReleaseReservation, releaseCmd, 'examples.ReleaseReservation');

      const releaseBook = {
        cover: {
          domain: 'inventory',
          root: eventBook.cover.root,
        },
        pages: [
          {
            sequence: 0,
            synchronous: false,
            command: releaseAny,
          },
        ],
        correlation_id: eventBook.correlation_id,
      };

      commands.push(releaseBook);

      const loyaltyPointsUsed = event.loyaltyPointsUsed || 0;
      if (loyaltyPointsUsed > 0) {
        const addPointsCmd = {
          points: loyaltyPointsUsed,
          reason: 'Order cancellation refund',
        };
        const addPointsAny = encodeToAny(AddLoyaltyPoints, addPointsCmd, 'examples.AddLoyaltyPoints');

        const addPointsBook = {
          cover: {
            domain: 'customer',
          },
          pages: [
            {
              sequence: 0,
              synchronous: false,
              command: addPointsAny,
            },
          ],
          correlation_id: eventBook.correlation_id,
        };

        commands.push(addPointsBook);
      }
    } catch (e) {
      logger.warn({ err: e, typeUrl }, 'failed to decode event');
    }
  }

  return commands;
}

const sagaService = {
  Handle(
    call: grpc.ServerUnaryCall<any, any>,
    callback: grpc.sendUnaryData<any>
  ) {
    try {
      const commands = processEvents(call.request);
      if (commands.length > 0) {
        logger.info({ compensationCommands: commands.length }, 'processed cancellation');
      }
      callback(null, {});
    } catch (err: any) {
      logger.error({ err }, 'error processing events');
      callback({
        code: grpc.status.INTERNAL,
        message: err.message || 'Internal error',
      });
    }
  },

  HandleSync(
    call: grpc.ServerUnaryCall<any, any>,
    callback: grpc.sendUnaryData<any>
  ) {
    try {
      const commands = processEvents(call.request);
      callback(null, { commands });
    } catch (err: any) {
      logger.error({ err }, 'error processing events');
      callback({
        code: grpc.status.INTERNAL,
        message: err.message || 'Internal error',
      });
    }
  },
};

async function main() {
  await loadProtoTypes();

  const port = process.env.PORT || '50409';
  const server = new grpc.Server();

  server.addService(grpcProto.angzarr.Saga.service, sagaService);

  const healthImpl = new HealthImplementation({ '': 'SERVING' });
  healthImpl.addToServer(server);

  server.bindAsync(
    `0.0.0.0:${port}`,
    grpc.ServerCredentials.createInsecure(),
    (err, boundPort) => {
      if (err) {
        logger.fatal({ err }, 'failed to bind server');
        process.exit(1);
      }
      logger.info({ saga: SAGA_NAME, port: boundPort, sourceDomain: SOURCE_DOMAIN }, 'saga server started');
    }
  );
}

main();
