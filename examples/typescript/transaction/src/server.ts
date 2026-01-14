/**
 * Transaction Service - TypeScript Implementation
 *
 * Handles purchases and discounts.
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
const DOMAIN = 'transaction';

// Load proto definitions for gRPC services
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

// Proto types
let TransactionCreated: protobuf.Type;
let DiscountApplied: protobuf.Type;
let TransactionCompleted: protobuf.Type;
let TransactionCancelled: protobuf.Type;
let TransactionState: protobuf.Type;
let CreateTransaction: protobuf.Type;
let ApplyDiscount: protobuf.Type;
let CompleteTransaction: protobuf.Type;
let CancelTransaction: protobuf.Type;
let LineItem: protobuf.Type;

async function loadProtoTypes() {
  const root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  TransactionCreated = root.lookupType('examples.TransactionCreated');
  DiscountApplied = root.lookupType('examples.DiscountApplied');
  TransactionCompleted = root.lookupType('examples.TransactionCompleted');
  TransactionCancelled = root.lookupType('examples.TransactionCancelled');
  TransactionState = root.lookupType('examples.TransactionState');
  CreateTransaction = root.lookupType('examples.CreateTransaction');
  ApplyDiscount = root.lookupType('examples.ApplyDiscount');
  CompleteTransaction = root.lookupType('examples.CompleteTransaction');
  CancelTransaction = root.lookupType('examples.CancelTransaction');
  LineItem = root.lookupType('examples.LineItem');
}

// Transaction state
interface ITransactionState {
  customerId: string;
  items: any[];
  subtotalCents: number;
  discountCents: number;
  discountType: string;
  status: string;
}

// Rebuild state from events
function rebuildState(eventBook: any): ITransactionState {
  const state: ITransactionState = {
    customerId: '',
    items: [],
    subtotalCents: 0,
    discountCents: 0,
    discountType: '',
    status: '',
  };

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = TransactionState.decode(eventBook.snapshot.state.value) as any;
      state.customerId = snapState.customerId || '';
      state.items = snapState.items || [];
      state.subtotalCents = snapState.subtotalCents || 0;
      state.discountCents = snapState.discountCents || 0;
      state.discountType = snapState.discountType || '';
      state.status = snapState.status || '';
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('TransactionCreated')) {
        const event = TransactionCreated.decode(page.event.value) as any;
        state.customerId = event.customerId;
        state.items = event.items || [];
        state.subtotalCents = event.subtotalCents;
        state.status = 'pending';
      } else if (typeUrl.endsWith('DiscountApplied')) {
        const event = DiscountApplied.decode(page.event.value) as any;
        state.discountCents = event.discountCents;
        state.discountType = event.discountType;
      } else if (typeUrl.endsWith('TransactionCompleted')) {
        state.status = 'completed';
      } else if (typeUrl.endsWith('TransactionCancelled')) {
        state.status = 'cancelled';
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

// Calculate subtotal from items
function calculateSubtotal(items: any[]): number {
  return items.reduce((sum, item) => sum + (item.quantity || 1) * (item.unitPriceCents || 0), 0);
}

function handleCreateTransaction(cmdBook: any, cmdData: Uint8Array, state: ITransactionState, seq: number): any {
  if (state.status) {
    throw { code: grpc.status.FAILED_PRECONDITION, message: 'Transaction already exists' };
  }

  const cmd = CreateTransaction.decode(cmdData) as any;

  if (!cmd.customerId) {
    throw { code: grpc.status.INVALID_ARGUMENT, message: 'Customer ID is required' };
  }
  if (!cmd.items?.length) {
    throw { code: grpc.status.INVALID_ARGUMENT, message: 'At least one item is required' };
  }

  const subtotal = calculateSubtotal(cmd.items);

  logger.info({ customerId: cmd.customerId, itemCount: cmd.items.length, subtotal }, 'creating transaction');

  const event = {
    customerId: cmd.customerId,
    items: cmd.items,
    subtotalCents: subtotal,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [{ num: seq, event: encodeToAny(TransactionCreated, event, 'examples.TransactionCreated'), created_at: now() }],
    },
  };
}

function handleApplyDiscount(cmdBook: any, cmdData: Uint8Array, state: ITransactionState, seq: number): any {
  if (state.status !== 'pending') {
    throw { code: grpc.status.FAILED_PRECONDITION, message: 'Transaction not in pending state' };
  }

  const cmd = ApplyDiscount.decode(cmdData) as any;

  let discountCents = 0;
  if (cmd.discountType === 'percentage') {
    discountCents = Math.floor((state.subtotalCents * cmd.value) / 100);
  } else if (cmd.discountType === 'fixed') {
    discountCents = cmd.value;
  }

  logger.info({ discountType: cmd.discountType, value: cmd.value, discountCents }, 'applying discount');

  const event = {
    discountType: cmd.discountType,
    value: cmd.value,
    discountCents: discountCents,
    couponCode: cmd.couponCode,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [{ num: seq, event: encodeToAny(DiscountApplied, event, 'examples.DiscountApplied'), created_at: now() }],
    },
  };
}

function handleCompleteTransaction(cmdBook: any, cmdData: Uint8Array, state: ITransactionState, seq: number): any {
  if (state.status !== 'pending') {
    throw { code: grpc.status.FAILED_PRECONDITION, message: 'Transaction not in pending state' };
  }

  const cmd = CompleteTransaction.decode(cmdData) as any;
  const finalTotal = state.subtotalCents - state.discountCents;
  const loyaltyPoints = Math.floor(finalTotal / 100); // 1 point per dollar

  logger.info({ finalTotal, loyaltyPoints, paymentMethod: cmd.paymentMethod }, 'completing transaction');

  const event = {
    finalTotalCents: finalTotal,
    paymentMethod: cmd.paymentMethod,
    loyaltyPointsEarned: loyaltyPoints,
    completedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [{ num: seq, event: encodeToAny(TransactionCompleted, event, 'examples.TransactionCompleted'), created_at: now() }],
    },
  };
}

function handleCancelTransaction(cmdBook: any, cmdData: Uint8Array, state: ITransactionState, seq: number): any {
  if (state.status !== 'pending') {
    throw { code: grpc.status.FAILED_PRECONDITION, message: 'Transaction not in pending state' };
  }

  const cmd = CancelTransaction.decode(cmdData) as any;

  logger.info({ reason: cmd.reason }, 'cancelling transaction');

  const event = {
    reason: cmd.reason,
    cancelledAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [{ num: seq, event: encodeToAny(TransactionCancelled, event, 'examples.TransactionCancelled'), created_at: now() }],
    },
  };
}

const businessLogicService = {
  Handle(call: grpc.ServerUnaryCall<any, any>, callback: grpc.sendUnaryData<any>) {
    try {
      const { command: cmdBook, events: priorEvents } = call.request;

      if (!cmdBook?.pages?.length) {
        callback({ code: grpc.status.INVALID_ARGUMENT, message: 'CommandBook has no pages' });
        return;
      }

      const cmdPage = cmdBook.pages[0];
      if (!cmdPage.command) {
        callback({ code: grpc.status.INVALID_ARGUMENT, message: 'Command page has no command' });
        return;
      }

      const state = rebuildState(priorEvents);
      const seq = nextSequence(priorEvents);
      const typeUrl = cmdPage.command.type_url || '';
      const cmdData = cmdPage.command.value;

      let response: any;

      if (typeUrl.endsWith('CreateTransaction')) {
        response = handleCreateTransaction(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ApplyDiscount')) {
        response = handleApplyDiscount(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('CompleteTransaction')) {
        response = handleCompleteTransaction(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('CancelTransaction')) {
        response = handleCancelTransaction(cmdBook, cmdData, state, seq);
      } else {
        callback({ code: grpc.status.INVALID_ARGUMENT, message: `Unknown command type: ${typeUrl}` });
        return;
      }

      callback(null, response);
    } catch (err: any) {
      if (err.code !== undefined) {
        callback(err);
      } else {
        logger.error({ err }, 'unexpected error');
        callback({ code: grpc.status.INTERNAL, message: err.message || 'Internal error' });
      }
    }
  },
};

async function main() {
  await loadProtoTypes();

  const port = process.env.PORT || '50053';
  const server = new grpc.Server();

  server.addService(grpcProto.angzarr.BusinessLogic.service, businessLogicService);

  const healthImpl = new HealthImplementation({ '': 'SERVING' });
  healthImpl.addToServer(server);

  server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
    if (err) {
      logger.fatal({ err }, 'failed to bind server');
      process.exit(1);
    }
    logger.info({ domain: DOMAIN, port: boundPort }, 'business logic server started');
  });
}

main();
