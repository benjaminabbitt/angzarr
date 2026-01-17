/**
 * Order Service - TypeScript Implementation
 *
 * Handles order lifecycle management.
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
const DOMAIN = 'order';

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
let OrderCreated: protobuf.Type;
let LoyaltyDiscountApplied: protobuf.Type;
let PaymentSubmitted: protobuf.Type;
let PaymentConfirmed: protobuf.Type;
let OrderCompleted: protobuf.Type;
let OrderCancelled: protobuf.Type;
let OrderState: protobuf.Type;
let CreateOrder: protobuf.Type;
let ApplyLoyaltyDiscount: protobuf.Type;
let SubmitPayment: protobuf.Type;
let ConfirmPayment: protobuf.Type;
let CancelOrder: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  OrderCreated = root.lookupType('examples.OrderCreated');
  LoyaltyDiscountApplied = root.lookupType('examples.LoyaltyDiscountApplied');
  PaymentSubmitted = root.lookupType('examples.PaymentSubmitted');
  PaymentConfirmed = root.lookupType('examples.PaymentConfirmed');
  OrderCompleted = root.lookupType('examples.OrderCompleted');
  OrderCancelled = root.lookupType('examples.OrderCancelled');
  OrderState = root.lookupType('examples.OrderState');
  CreateOrder = root.lookupType('examples.CreateOrder');
  ApplyLoyaltyDiscount = root.lookupType('examples.ApplyLoyaltyDiscount');
  SubmitPayment = root.lookupType('examples.SubmitPayment');
  ConfirmPayment = root.lookupType('examples.ConfirmPayment');
  CancelOrder = root.lookupType('examples.CancelOrder');
}

interface LineItem {
  productId: string;
  name: string;
  quantity: number;
  unitPriceCents: number;
}

interface IOrderState {
  customerId: string;
  items: LineItem[];
  subtotalCents: number;
  discountCents: number;
  loyaltyPointsUsed: number;
  paymentMethod: string;
  paymentReference: string;
  status: string;
}

function emptyState(): IOrderState {
  return {
    customerId: '',
    items: [],
    subtotalCents: 0,
    discountCents: 0,
    loyaltyPointsUsed: 0,
    paymentMethod: '',
    paymentReference: '',
    status: '',
  };
}

function rebuildState(eventBook: any): IOrderState {
  const state = emptyState();

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = OrderState.decode(eventBook.snapshot.state.value) as any;
      state.customerId = snapState.customerId || '';
      state.subtotalCents = snapState.subtotalCents || 0;
      state.discountCents = snapState.discountCents || 0;
      state.loyaltyPointsUsed = snapState.loyaltyPointsUsed || 0;
      state.paymentMethod = snapState.paymentMethod || '';
      state.paymentReference = snapState.paymentReference || '';
      state.status = snapState.status || '';
      if (snapState.items) {
        state.items = snapState.items.map((item: any) => ({
          productId: item.productId,
          name: item.name,
          quantity: item.quantity,
          unitPriceCents: item.unitPriceCents,
        }));
      }
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('OrderCreated')) {
        const event = OrderCreated.decode(page.event.value) as any;
        state.customerId = event.customerId;
        state.items = (event.items || []).map((item: any) => ({
          productId: item.productId,
          name: item.name,
          quantity: item.quantity,
          unitPriceCents: item.unitPriceCents,
        }));
        state.subtotalCents = event.subtotalCents;
        state.discountCents = event.discountCents || 0;
        state.status = 'pending';
      } else if (typeUrl.endsWith('LoyaltyDiscountApplied')) {
        const event = LoyaltyDiscountApplied.decode(page.event.value) as any;
        state.loyaltyPointsUsed = event.pointsUsed;
        state.discountCents += event.discountCents;
      } else if (typeUrl.endsWith('PaymentSubmitted')) {
        const event = PaymentSubmitted.decode(page.event.value) as any;
        state.paymentMethod = event.paymentMethod;
        state.paymentReference = event.paymentReference;
        state.status = 'payment_submitted';
      } else if (typeUrl.endsWith('PaymentConfirmed')) {
        state.status = 'payment_confirmed';
      } else if (typeUrl.endsWith('OrderCompleted')) {
        state.status = 'completed';
      } else if (typeUrl.endsWith('OrderCancelled')) {
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

function handleCreateOrder(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IOrderState,
  seq: number
): any {
  if (state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order already exists',
    };
  }

  const cmd = CreateOrder.decode(cmdData) as any;

  if (!cmd.customerId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Customer ID is required',
    };
  }
  if (!cmd.items || cmd.items.length === 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Order must have at least one item',
    };
  }

  logger.info({ customerId: cmd.customerId, itemCount: cmd.items.length }, 'creating order');

  const event = {
    customerId: cmd.customerId,
    items: cmd.items,
    subtotalCents: cmd.subtotalCents,
    discountCents: cmd.discountCents || 0,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(OrderCreated, event, 'examples.OrderCreated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleApplyLoyaltyDiscount(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IOrderState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order does not exist',
    };
  }
  if (state.status !== 'pending') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot apply discount to non-pending order',
    };
  }
  if (state.loyaltyPointsUsed > 0) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Loyalty discount already applied',
    };
  }

  const cmd = ApplyLoyaltyDiscount.decode(cmdData) as any;

  if (cmd.points <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Points must be positive',
    };
  }

  const discountCents = cmd.points;

  logger.info({ points: cmd.points, discountCents }, 'applying loyalty discount');

  const event = {
    pointsUsed: cmd.points,
    discountCents: discountCents,
    appliedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(LoyaltyDiscountApplied, event, 'examples.LoyaltyDiscountApplied'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleSubmitPayment(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IOrderState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order does not exist',
    };
  }
  if (state.status !== 'pending') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Payment already submitted or order not pending',
    };
  }

  const cmd = SubmitPayment.decode(cmdData) as any;

  if (!cmd.paymentMethod) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Payment method is required',
    };
  }

  const totalCents = state.subtotalCents - state.discountCents;

  logger.info({ paymentMethod: cmd.paymentMethod, totalCents }, 'submitting payment');

  const event = {
    paymentMethod: cmd.paymentMethod,
    paymentReference: cmd.paymentReference || '',
    amountCents: totalCents,
    submittedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(PaymentSubmitted, event, 'examples.PaymentSubmitted'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleConfirmPayment(
  cmdBook: any,
  _cmdData: Uint8Array,
  state: IOrderState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order does not exist',
    };
  }
  if (state.status !== 'payment_submitted') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Payment not submitted',
    };
  }

  logger.info({ customerId: state.customerId }, 'confirming payment');

  const event = {
    confirmedAt: now(),
  };

  const pages = [
    {
      num: seq,
      event: encodeToAny(PaymentConfirmed, event, 'examples.PaymentConfirmed'),
      created_at: now(),
    },
    {
      num: seq + 1,
      event: encodeToAny(OrderCompleted, { completedAt: now() }, 'examples.OrderCompleted'),
      created_at: now(),
    },
  ];

  return {
    events: {
      cover: cmdBook.cover,
      pages: pages,
    },
  };
}

function handleCancelOrder(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IOrderState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order does not exist',
    };
  }
  if (state.status === 'cancelled') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Order already cancelled',
    };
  }
  if (state.status === 'completed') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot cancel completed order',
    };
  }

  const cmd = CancelOrder.decode(cmdData) as any;

  logger.info({ customerId: state.customerId, reason: cmd.reason }, 'cancelling order');

  const event = {
    reason: cmd.reason || '',
    loyaltyPointsUsed: state.loyaltyPointsUsed,
    cancelledAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(OrderCancelled, event, 'examples.OrderCancelled'),
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

      if (typeUrl.endsWith('CreateOrder')) {
        response = handleCreateOrder(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ApplyLoyaltyDiscount')) {
        response = handleApplyLoyaltyDiscount(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('SubmitPayment')) {
        response = handleSubmitPayment(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ConfirmPayment')) {
        response = handleConfirmPayment(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('CancelOrder')) {
        response = handleCancelOrder(cmdBook, cmdData, state, seq);
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

  const port = process.env.PORT || '50403';
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
