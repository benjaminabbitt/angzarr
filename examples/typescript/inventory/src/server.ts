/**
 * Inventory Service - TypeScript Implementation
 *
 * Handles inventory stock management.
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
const DOMAIN = 'inventory';

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
let StockInitialized: protobuf.Type;
let StockReceived: protobuf.Type;
let StockReserved: protobuf.Type;
let ReservationReleased: protobuf.Type;
let ReservationCommitted: protobuf.Type;
let LowStockAlert: protobuf.Type;
let InventoryState: protobuf.Type;
let InitializeStock: protobuf.Type;
let ReceiveStock: protobuf.Type;
let ReserveStock: protobuf.Type;
let ReleaseReservation: protobuf.Type;
let CommitReservation: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  StockInitialized = root.lookupType('examples.StockInitialized');
  StockReceived = root.lookupType('examples.StockReceived');
  StockReserved = root.lookupType('examples.StockReserved');
  ReservationReleased = root.lookupType('examples.ReservationReleased');
  ReservationCommitted = root.lookupType('examples.ReservationCommitted');
  LowStockAlert = root.lookupType('examples.LowStockAlert');
  InventoryState = root.lookupType('examples.InventoryState');
  InitializeStock = root.lookupType('examples.InitializeStock');
  ReceiveStock = root.lookupType('examples.ReceiveStock');
  ReserveStock = root.lookupType('examples.ReserveStock');
  ReleaseReservation = root.lookupType('examples.ReleaseReservation');
  CommitReservation = root.lookupType('examples.CommitReservation');
}

interface IInventoryState {
  productId: string;
  onHand: number;
  reserved: number;
  lowStockThreshold: number;
  reservations: Map<string, number>;
}

function emptyState(): IInventoryState {
  return {
    productId: '',
    onHand: 0,
    reserved: 0,
    lowStockThreshold: 0,
    reservations: new Map(),
  };
}

function available(state: IInventoryState): number {
  return state.onHand - state.reserved;
}

function rebuildState(eventBook: any): IInventoryState {
  const state = emptyState();

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = InventoryState.decode(eventBook.snapshot.state.value) as any;
      state.productId = snapState.productId || '';
      state.onHand = snapState.onHand || 0;
      state.reserved = snapState.reserved || 0;
      state.lowStockThreshold = snapState.lowStockThreshold || 0;
      if (snapState.reservations) {
        for (const [key, value] of Object.entries(snapState.reservations)) {
          state.reservations.set(key, value as number);
        }
      }
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('StockInitialized')) {
        const event = StockInitialized.decode(page.event.value) as any;
        state.productId = event.productId;
        state.onHand = event.initialQuantity;
        state.lowStockThreshold = event.lowStockThreshold;
      } else if (typeUrl.endsWith('StockReceived')) {
        const event = StockReceived.decode(page.event.value) as any;
        state.onHand += event.quantity;
      } else if (typeUrl.endsWith('StockReserved')) {
        const event = StockReserved.decode(page.event.value) as any;
        state.reserved += event.quantity;
        state.reservations.set(event.orderId, event.quantity);
      } else if (typeUrl.endsWith('ReservationReleased')) {
        const event = ReservationReleased.decode(page.event.value) as any;
        const qty = state.reservations.get(event.orderId) || 0;
        state.reserved -= qty;
        state.reservations.delete(event.orderId);
      } else if (typeUrl.endsWith('ReservationCommitted')) {
        const event = ReservationCommitted.decode(page.event.value) as any;
        const qty = state.reservations.get(event.orderId) || 0;
        state.onHand -= qty;
        state.reserved -= qty;
        state.reservations.delete(event.orderId);
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

function handleInitializeStock(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IInventoryState,
  seq: number
): any {
  if (state.productId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Inventory already initialized',
    };
  }

  const cmd = InitializeStock.decode(cmdData) as any;

  if (!cmd.productId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product ID is required',
    };
  }
  if (cmd.initialQuantity < 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Initial quantity cannot be negative',
    };
  }

  logger.info({ productId: cmd.productId, quantity: cmd.initialQuantity }, 'initializing stock');

  const event = {
    productId: cmd.productId,
    initialQuantity: cmd.initialQuantity,
    lowStockThreshold: cmd.lowStockThreshold || 10,
    initializedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(StockInitialized, event, 'examples.StockInitialized'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleReceiveStock(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IInventoryState,
  seq: number
): any {
  if (!state.productId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Inventory not initialized',
    };
  }

  const cmd = ReceiveStock.decode(cmdData) as any;

  if (cmd.quantity <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Quantity must be positive',
    };
  }

  logger.info({ productId: state.productId, quantity: cmd.quantity }, 'receiving stock');

  const event = {
    quantity: cmd.quantity,
    newOnHand: state.onHand + cmd.quantity,
    receivedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(StockReceived, event, 'examples.StockReceived'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleReserveStock(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IInventoryState,
  seq: number
): any {
  if (!state.productId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Inventory not initialized',
    };
  }

  const cmd = ReserveStock.decode(cmdData) as any;

  if (!cmd.orderId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Order ID is required',
    };
  }
  if (cmd.quantity <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Quantity must be positive',
    };
  }
  if (state.reservations.has(cmd.orderId)) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Reservation already exists for order',
    };
  }

  const avail = available(state);
  if (cmd.quantity > avail) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Insufficient stock: available ${avail}, requested ${cmd.quantity}`,
    };
  }

  logger.info({ productId: state.productId, orderId: cmd.orderId, quantity: cmd.quantity }, 'reserving stock');

  const pages = [];

  const reserveEvent = {
    orderId: cmd.orderId,
    quantity: cmd.quantity,
    reservedAt: now(),
  };
  pages.push({
    num: seq,
    event: encodeToAny(StockReserved, reserveEvent, 'examples.StockReserved'),
    created_at: now(),
  });

  const newAvailable = avail - cmd.quantity;
  if (newAvailable <= state.lowStockThreshold) {
    const alertEvent = {
      availableQuantity: newAvailable,
      threshold: state.lowStockThreshold,
      alertedAt: now(),
    };
    pages.push({
      num: seq + 1,
      event: encodeToAny(LowStockAlert, alertEvent, 'examples.LowStockAlert'),
      created_at: now(),
    });
  }

  return {
    events: {
      cover: cmdBook.cover,
      pages: pages,
    },
  };
}

function handleReleaseReservation(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IInventoryState,
  seq: number
): any {
  if (!state.productId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Inventory not initialized',
    };
  }

  const cmd = ReleaseReservation.decode(cmdData) as any;

  if (!cmd.orderId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Order ID is required',
    };
  }
  if (!state.reservations.has(cmd.orderId)) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Reservation not found',
    };
  }

  const releasedQty = state.reservations.get(cmd.orderId)!;

  logger.info({ productId: state.productId, orderId: cmd.orderId, quantity: releasedQty }, 'releasing reservation');

  const event = {
    orderId: cmd.orderId,
    releasedQuantity: releasedQty,
    releasedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ReservationReleased, event, 'examples.ReservationReleased'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleCommitReservation(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IInventoryState,
  seq: number
): any {
  if (!state.productId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Inventory not initialized',
    };
  }

  const cmd = CommitReservation.decode(cmdData) as any;

  if (!cmd.orderId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Order ID is required',
    };
  }
  if (!state.reservations.has(cmd.orderId)) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Reservation not found',
    };
  }

  const committedQty = state.reservations.get(cmd.orderId)!;

  logger.info({ productId: state.productId, orderId: cmd.orderId, quantity: committedQty }, 'committing reservation');

  const event = {
    orderId: cmd.orderId,
    committedQuantity: committedQty,
    committedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ReservationCommitted, event, 'examples.ReservationCommitted'),
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

      if (typeUrl.endsWith('InitializeStock')) {
        response = handleInitializeStock(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ReceiveStock')) {
        response = handleReceiveStock(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ReserveStock')) {
        response = handleReserveStock(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ReleaseReservation')) {
        response = handleReleaseReservation(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('CommitReservation')) {
        response = handleCommitReservation(cmdBook, cmdData, state, seq);
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

  const port = process.env.PORT || '50404';
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
