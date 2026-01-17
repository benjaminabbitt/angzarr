/**
 * Loyalty Earn Saga - TypeScript Implementation
 *
 * Listens for Delivered events and awards loyalty points.
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
const SAGA_NAME = 'loyalty-earn';
const SOURCE_DOMAIN = 'fulfillment';
const POINTS_PER_DOLLAR = 10;

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
let Delivered: protobuf.Type;
let AddLoyaltyPoints: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  Delivered = root.lookupType('examples.Delivered');
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

    if (!typeUrl.endsWith('Delivered')) {
      continue;
    }

    let orderId = '';
    if (eventBook.cover?.root?.value) {
      const rootBytes = eventBook.cover.root.value;
      orderId = Buffer.from(rootBytes).toString('hex');
    }

    if (!orderId) continue;

    const points = POINTS_PER_DOLLAR * 100;

    logger.info({ orderId, points }, 'awarding loyalty points for delivery');

    const addPointsCmd = {
      points,
      reason: `Order delivery: ${orderId}`,
    };
    const cmdAny = encodeToAny(AddLoyaltyPoints, addPointsCmd, 'examples.AddLoyaltyPoints');

    const commandBook = {
      cover: {
        domain: 'customer',
      },
      pages: [
        {
          sequence: 0,
          synchronous: false,
          command: cmdAny,
        },
      ],
      correlation_id: eventBook.correlation_id,
    };

    commands.push(commandBook);
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
        logger.info({ commandCount: commands.length }, 'processed delivery for loyalty points');
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

  const port = process.env.PORT || '50408';
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
