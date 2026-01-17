/**
 * Cart Service - TypeScript Implementation
 *
 * Handles shopping cart operations.
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
const DOMAIN = 'cart';

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
let CartCreated: protobuf.Type;
let ItemAdded: protobuf.Type;
let QuantityUpdated: protobuf.Type;
let ItemRemoved: protobuf.Type;
let CouponApplied: protobuf.Type;
let CartCleared: protobuf.Type;
let CartCheckedOut: protobuf.Type;
let CartState: protobuf.Type;
let CreateCart: protobuf.Type;
let AddItem: protobuf.Type;
let UpdateQuantity: protobuf.Type;
let RemoveItem: protobuf.Type;
let ApplyCoupon: protobuf.Type;
let ClearCart: protobuf.Type;
let Checkout: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  CartCreated = root.lookupType('examples.CartCreated');
  ItemAdded = root.lookupType('examples.ItemAdded');
  QuantityUpdated = root.lookupType('examples.QuantityUpdated');
  ItemRemoved = root.lookupType('examples.ItemRemoved');
  CouponApplied = root.lookupType('examples.CouponApplied');
  CartCleared = root.lookupType('examples.CartCleared');
  CartCheckedOut = root.lookupType('examples.CartCheckedOut');
  CartState = root.lookupType('examples.CartState');
  CreateCart = root.lookupType('examples.CreateCart');
  AddItem = root.lookupType('examples.AddItem');
  UpdateQuantity = root.lookupType('examples.UpdateQuantity');
  RemoveItem = root.lookupType('examples.RemoveItem');
  ApplyCoupon = root.lookupType('examples.ApplyCoupon');
  ClearCart = root.lookupType('examples.ClearCart');
  Checkout = root.lookupType('examples.Checkout');
}

interface CartItem {
  productId: string;
  name: string;
  quantity: number;
  unitPriceCents: number;
}

interface ICartState {
  customerId: string;
  items: Map<string, CartItem>;
  subtotalCents: number;
  couponCode: string;
  discountCents: number;
  status: string;
}

function emptyState(): ICartState {
  return {
    customerId: '',
    items: new Map(),
    subtotalCents: 0,
    couponCode: '',
    discountCents: 0,
    status: '',
  };
}

function calculateSubtotal(items: Map<string, CartItem>): number {
  let total = 0;
  for (const item of items.values()) {
    total += item.quantity * item.unitPriceCents;
  }
  return total;
}

function rebuildState(eventBook: any): ICartState {
  const state = emptyState();

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = CartState.decode(eventBook.snapshot.state.value) as any;
      state.customerId = snapState.customerId || '';
      state.subtotalCents = snapState.subtotalCents || 0;
      state.couponCode = snapState.couponCode || '';
      state.discountCents = snapState.discountCents || 0;
      state.status = snapState.status || '';
      if (snapState.items) {
        for (const item of snapState.items) {
          state.items.set(item.productId, {
            productId: item.productId,
            name: item.name,
            quantity: item.quantity,
            unitPriceCents: item.unitPriceCents,
          });
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
      if (typeUrl.endsWith('CartCreated')) {
        const event = CartCreated.decode(page.event.value) as any;
        state.customerId = event.customerId;
        state.status = 'active';
      } else if (typeUrl.endsWith('ItemAdded')) {
        const event = ItemAdded.decode(page.event.value) as any;
        state.items.set(event.productId, {
          productId: event.productId,
          name: event.name,
          quantity: event.quantity,
          unitPriceCents: event.unitPriceCents,
        });
        state.subtotalCents = calculateSubtotal(state.items);
      } else if (typeUrl.endsWith('QuantityUpdated')) {
        const event = QuantityUpdated.decode(page.event.value) as any;
        const item = state.items.get(event.productId);
        if (item) {
          item.quantity = event.newQuantity;
          state.subtotalCents = calculateSubtotal(state.items);
        }
      } else if (typeUrl.endsWith('ItemRemoved')) {
        const event = ItemRemoved.decode(page.event.value) as any;
        state.items.delete(event.productId);
        state.subtotalCents = calculateSubtotal(state.items);
      } else if (typeUrl.endsWith('CouponApplied')) {
        const event = CouponApplied.decode(page.event.value) as any;
        state.couponCode = event.couponCode;
        state.discountCents = event.discountCents;
      } else if (typeUrl.endsWith('CartCleared')) {
        state.items.clear();
        state.subtotalCents = 0;
        state.couponCode = '';
        state.discountCents = 0;
      } else if (typeUrl.endsWith('CartCheckedOut')) {
        state.status = 'checked_out';
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

function handleCreateCart(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart already exists',
    };
  }

  const cmd = CreateCart.decode(cmdData) as any;

  if (!cmd.customerId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Customer ID is required',
    };
  }

  logger.info({ customerId: cmd.customerId }, 'creating cart');

  const event = {
    customerId: cmd.customerId,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(CartCreated, event, 'examples.CartCreated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleAddItem(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot modify checked out cart',
    };
  }

  const cmd = AddItem.decode(cmdData) as any;

  if (!cmd.productId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product ID is required',
    };
  }
  if (cmd.quantity <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Quantity must be positive',
    };
  }
  if (cmd.unitPriceCents <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Unit price must be positive',
    };
  }

  logger.info({ productId: cmd.productId, quantity: cmd.quantity }, 'adding item to cart');

  const event = {
    productId: cmd.productId,
    name: cmd.name || '',
    quantity: cmd.quantity,
    unitPriceCents: cmd.unitPriceCents,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ItemAdded, event, 'examples.ItemAdded'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleUpdateQuantity(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot modify checked out cart',
    };
  }

  const cmd = UpdateQuantity.decode(cmdData) as any;

  if (!cmd.productId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product ID is required',
    };
  }
  if (cmd.newQuantity <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Quantity must be positive',
    };
  }
  if (!state.items.has(cmd.productId)) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Item not in cart',
    };
  }

  const item = state.items.get(cmd.productId)!;

  logger.info({ productId: cmd.productId, oldQuantity: item.quantity, newQuantity: cmd.newQuantity }, 'updating quantity');

  const event = {
    productId: cmd.productId,
    oldQuantity: item.quantity,
    newQuantity: cmd.newQuantity,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(QuantityUpdated, event, 'examples.QuantityUpdated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleRemoveItem(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot modify checked out cart',
    };
  }

  const cmd = RemoveItem.decode(cmdData) as any;

  if (!cmd.productId) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product ID is required',
    };
  }
  if (!state.items.has(cmd.productId)) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Item not in cart',
    };
  }

  logger.info({ productId: cmd.productId }, 'removing item from cart');

  const event = {
    productId: cmd.productId,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ItemRemoved, event, 'examples.ItemRemoved'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleApplyCoupon(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot modify checked out cart',
    };
  }
  if (state.couponCode) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Coupon already applied',
    };
  }

  const cmd = ApplyCoupon.decode(cmdData) as any;

  if (!cmd.couponCode) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Coupon code is required',
    };
  }
  if (cmd.discountCents <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Discount must be positive',
    };
  }

  logger.info({ couponCode: cmd.couponCode, discountCents: cmd.discountCents }, 'applying coupon');

  const event = {
    couponCode: cmd.couponCode,
    discountCents: cmd.discountCents,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(CouponApplied, event, 'examples.CouponApplied'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleClearCart(
  cmdBook: any,
  _cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot modify checked out cart',
    };
  }

  logger.info({ customerId: state.customerId }, 'clearing cart');

  const event = {
    clearedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(CartCleared, event, 'examples.CartCleared'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleCheckout(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICartState,
  seq: number
): any {
  if (!state.customerId) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart does not exist',
    };
  }
  if (state.status === 'checked_out') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cart already checked out',
    };
  }
  if (state.items.size === 0) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot checkout empty cart',
    };
  }

  const cmd = Checkout.decode(cmdData) as any;

  logger.info({ customerId: state.customerId, itemCount: state.items.size }, 'checking out cart');

  const items = Array.from(state.items.values()).map(item => ({
    productId: item.productId,
    name: item.name,
    quantity: item.quantity,
    unitPriceCents: item.unitPriceCents,
  }));

  const event = {
    customerId: state.customerId,
    items: items,
    subtotalCents: state.subtotalCents,
    discountCents: state.discountCents,
    totalCents: state.subtotalCents - state.discountCents,
    loyaltyPointsToUse: cmd.loyaltyPointsToUse || 0,
    checkedOutAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(CartCheckedOut, event, 'examples.CartCheckedOut'),
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

      if (typeUrl.endsWith('CreateCart')) {
        response = handleCreateCart(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('AddItem')) {
        response = handleAddItem(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('UpdateQuantity')) {
        response = handleUpdateQuantity(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('RemoveItem')) {
        response = handleRemoveItem(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ApplyCoupon')) {
        response = handleApplyCoupon(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('ClearCart')) {
        response = handleClearCart(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('Checkout')) {
        response = handleCheckout(cmdBook, cmdData, state, seq);
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

  const port = process.env.PORT || '50402';
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
