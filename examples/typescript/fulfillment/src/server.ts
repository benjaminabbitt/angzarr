/**
 * Fulfillment Service - TypeScript Implementation
 *
 * Handles shipment state machine: pending -> picking -> packing -> shipped -> delivered
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
const DOMAIN = 'fulfillment';

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
let ShipmentCreated: protobuf.Type;
let ItemsPicked: protobuf.Type;
let ItemsPacked: protobuf.Type;
let Shipped: protobuf.Type;
let Delivered: protobuf.Type;
let FulfillmentState: protobuf.Type;
let CreateShipment: protobuf.Type;
let MarkPicked: protobuf.Type;
let MarkPacked: protobuf.Type;
let Ship: protobuf.Type;
let RecordDelivery: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  ShipmentCreated = root.lookupType('examples.ShipmentCreated');
  ItemsPicked = root.lookupType('examples.ItemsPicked');
  ItemsPacked = root.lookupType('examples.ItemsPacked');
  Shipped = root.lookupType('examples.Shipped');
  Delivered = root.lookupType('examples.Delivered');
  FulfillmentState = root.lookupType('examples.FulfillmentState');
  CreateShipment = root.lookupType('examples.CreateShipment');
  MarkPicked = root.lookupType('examples.MarkPicked');
  MarkPacked = root.lookupType('examples.MarkPacked');
  Ship = root.lookupType('examples.Ship');
  RecordDelivery = root.lookupType('examples.RecordDelivery');
}

interface IFulfillmentState {
  orderId: string;
  status: string;
  trackingNumber: string;
  carrier: string;
  pickerId: string;
  packerId: string;
  signature: string;
}

function emptyState(): IFulfillmentState {
  return {
    orderId: '',
    status: '',
    trackingNumber: '',
    carrier: '',
    pickerId: '',
    packerId: '',
    signature: '',
  };
}

function rebuildState(eventBook: any): IFulfillmentState {
  const state = emptyState();

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = FulfillmentState.decode(eventBook.snapshot.state.value) as any;
      state.orderId = snapState.orderId || '';
      state.status = snapState.status || '';
      state.trackingNumber = snapState.trackingNumber || '';
      state.carrier = snapState.carrier || '';
      state.pickerId = snapState.pickerId || '';
      state.packerId = snapState.packerId || '';
      state.signature = snapState.signature || '';
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('ShipmentCreated')) {
        const event = ShipmentCreated.decode(page.event.value) as any;
        state.orderId = event.orderId;
        state.status = 'pending';
      } else if (typeUrl.endsWith('ItemsPicked')) {
        const event = ItemsPicked.decode(page.event.value) as any;
        state.status = 'picking';
        state.pickerId = event.pickerId;
      } else if (typeUrl.endsWith('ItemsPacked')) {
        const event = ItemsPacked.decode(page.event.value) as any;
        state.status = 'packing';
        state.packerId = event.packerId;
      } else if (typeUrl.endsWith('Shipped')) {
        const event = Shipped.decode(page.event.value) as any;
        state.status = 'shipped';
        state.trackingNumber = event.trackingNumber;
        state.carrier = event.carrier;
      } else if (typeUrl.endsWith('Delivered')) {
        const event = Delivered.decode(page.event.value) as any;
        state.status = 'delivered';
        state.signature = event.signature || '';
      }
    } catch (e) {
      logger.warn({ err: e, typeUrl }, 'failed to decode event');
    }
  }

  return state;
}

function encodeToAny(messageType: protobuf.Type, message: any, typeName: string): any {
  const encoded = messageType.encode(messageType.create(message)).finish();
  return {
    type_url: `type.examples/${typeName}`,
    value: Buffer.from(encoded),
  };
}

function nextSequence(priorEvents: any): number {
  if (!priorEvents?.pages?.length) return 0;
  return priorEvents.pages.length;
}

function now() {
  return { seconds: Math.floor(Date.now() / 1000), nanos: 0 };
}

function handleCreateShipment(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IFulfillmentState,
  seq: number
): any {
  if (state.orderId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Shipment already exists',
    };
  }

  const cmd = CreateShipment.decode(cmdData) as any;

  if (!cmd.orderId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Order ID is required',
    };
  }

  logger.info({ orderId: cmd.orderId }, 'creating shipment');

  const event = {
    orderId: cmd.orderId,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ShipmentCreated, event, 'examples.ShipmentCreated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleMarkPicked(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IFulfillmentState,
  seq: number
): any {
  if (!state.orderId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Shipment does not exist',
    };
  }
  if (state.status !== 'pending') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Cannot pick from status: ${state.status}`,
    };
  }

  const cmd = MarkPicked.decode(cmdData) as any;

  if (!cmd.pickerId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Picker ID is required',
    };
  }

  logger.info({ orderId: state.orderId, pickerId: cmd.pickerId }, 'marking items picked');

  const event = {
    pickerId: cmd.pickerId,
    pickedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ItemsPicked, event, 'examples.ItemsPicked'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleMarkPacked(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IFulfillmentState,
  seq: number
): any {
  if (!state.orderId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Shipment does not exist',
    };
  }
  if (state.status !== 'picking') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Cannot pack from status: ${state.status}`,
    };
  }

  const cmd = MarkPacked.decode(cmdData) as any;

  if (!cmd.packerId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Packer ID is required',
    };
  }

  logger.info({ orderId: state.orderId, packerId: cmd.packerId }, 'marking items packed');

  const event = {
    packerId: cmd.packerId,
    packedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ItemsPacked, event, 'examples.ItemsPacked'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleShip(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IFulfillmentState,
  seq: number
): any {
  if (!state.orderId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Shipment does not exist',
    };
  }
  if (state.status !== 'packing') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Cannot ship from status: ${state.status}`,
    };
  }

  const cmd = Ship.decode(cmdData) as any;

  if (!cmd.trackingNumber) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Tracking number is required',
    };
  }
  if (!cmd.carrier) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Carrier is required',
    };
  }

  logger.info({ orderId: state.orderId, carrier: cmd.carrier, tracking: cmd.trackingNumber }, 'shipping order');

  const event = {
    trackingNumber: cmd.trackingNumber,
    carrier: cmd.carrier,
    shippedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(Shipped, event, 'examples.Shipped'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleRecordDelivery(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IFulfillmentState,
  seq: number
): any {
  if (!state.orderId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Shipment does not exist',
    };
  }
  if (state.status !== 'shipped') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Cannot record delivery from status: ${state.status}`,
    };
  }

  const cmd = RecordDelivery.decode(cmdData) as any;

  logger.info({ orderId: state.orderId, signature: cmd.signature }, 'recording delivery');

  const event = {
    deliveredAt: now(),
    signature: cmd.signature || '',
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(Delivered, event, 'examples.Delivered'),
          created_at: now(),
        },
      ],
    },
  };
}

const businessLogicService = {
  Handle(
    call: grpc.ServerUnaryCall<any, any>,
    callback: grpc.sendUnaryData<any>
  ) {
    try {
      const { command: cmdBook, events: priorEvents } = call.request;

      if (!cmdBook?.pages?.length) {
        callback({
          code: grpc.status.INVALID_ARGUMENT,
          message: 'CommandBook has no pages',
        });
        return;
      }

      const cmdPage = cmdBook.pages[0];
      if (!cmdPage.command) {
        callback({
          code: grpc.status.INVALID_ARGUMENT,
          message: 'Command page has no command',
        });
        return;
      }

      const state = rebuildState(priorEvents);
      const seq = nextSequence(priorEvents);
      const typeUrl = cmdPage.command.type_url || '';
      const cmdData = cmdPage.command.value;

      let response: any;

      if (typeUrl.endsWith('CreateShipment')) {
        response = handleCreateShipment(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('MarkPicked')) {
        response = handleMarkPicked(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('MarkPacked')) {
        response = handleMarkPacked(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('Ship')) {
        response = handleShip(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('RecordDelivery')) {
        response = handleRecordDelivery(cmdBook, cmdData, state, seq);
      } else {
        callback({
          code: grpc.status.INVALID_ARGUMENT,
          message: `Unknown command type: ${typeUrl}`,
        });
        return;
      }

      callback(null, response);
    } catch (err: any) {
      if (err.code !== undefined) {
        callback(err);
      } else {
        logger.error({ err }, 'unexpected error');
        callback({
          code: grpc.status.INTERNAL,
          message: err.message || 'Internal error',
        });
      }
    }
  },
};

async function main() {
  await loadProtoTypes();

  const port = process.env.PORT || '50405';
  const server = new grpc.Server();

  server.addService(grpcProto.angzarr.BusinessLogic.service, businessLogicService);

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
      logger.info({ domain: DOMAIN, port: boundPort }, 'business logic server started');
    }
  );
}

main();
