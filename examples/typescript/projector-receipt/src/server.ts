/**
 * Receipt Projector - TypeScript Implementation
 *
 * Generates Receipt projections from order events.
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
const PROJECTOR_NAME = 'receipt';

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

let OrderCreated: protobuf.Type;
let LoyaltyDiscountApplied: protobuf.Type;
let PaymentSubmitted: protobuf.Type;
let OrderCompleted: protobuf.Type;
let Receipt: protobuf.Type;

async function loadProtoTypes() {
  const root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  OrderCreated = root.lookupType('examples.OrderCreated');
  LoyaltyDiscountApplied = root.lookupType('examples.LoyaltyDiscountApplied');
  PaymentSubmitted = root.lookupType('examples.PaymentSubmitted');
  OrderCompleted = root.lookupType('examples.OrderCompleted');
  Receipt = root.lookupType('examples.Receipt');
}

function uuidToHex(uuid: any): string {
  if (!uuid?.value) return '';
  return Buffer.from(uuid.value).toString('hex');
}

function encodeToAny(messageType: protobuf.Type, message: any, typeName: string): any {
  const encoded = messageType.encode(messageType.create(message)).finish();
  return {
    type_url: `type.examples/${typeName}`,
    value: Buffer.from(encoded),
  };
}

function formatCents(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

function formatReceipt(receipt: any): string {
  const lines: string[] = [];
  const width = 40;

  lines.push('='.repeat(width));
  lines.push('         RECEIPT');
  lines.push('='.repeat(width));
  lines.push('');

  for (const item of receipt.items || []) {
    const qty = item.quantity || 1;
    const price = formatCents(item.unitPriceCents || 0);
    const total = formatCents((item.unitPriceCents || 0) * qty);
    lines.push(`${item.name}`);
    lines.push(`  ${qty} x ${price} = ${total}`);
  }

  lines.push('-'.repeat(width));
  lines.push(`Subtotal: ${formatCents(receipt.subtotalCents || 0)}`);

  if (receipt.discountCents > 0) {
    lines.push(`Discount: -${formatCents(receipt.discountCents)}`);
  }

  lines.push(`Total: ${formatCents(receipt.finalTotalCents || 0)}`);
  lines.push('');
  lines.push(`Payment: ${receipt.paymentMethod || 'N/A'}`);

  if (receipt.loyaltyPointsEarned > 0) {
    lines.push(`Loyalty Points Earned: ${receipt.loyaltyPointsEarned}`);
  }

  lines.push('='.repeat(width));
  lines.push('       Thank you!');
  lines.push('='.repeat(width));

  return lines.join('\n');
}

interface ReceiptState {
  customerId: string;
  items: any[];
  subtotalCents: number;
  discountCents: number;
  loyaltyPointsUsed: number;
  finalTotalCents: number;
  paymentMethod: string;
  loyaltyPointsEarned: number;
  completedAt: any;
  isCompleted: boolean;
}

function buildReceiptState(eventBook: any): ReceiptState {
  const state: ReceiptState = {
    customerId: '',
    items: [],
    subtotalCents: 0,
    discountCents: 0,
    loyaltyPointsUsed: 0,
    finalTotalCents: 0,
    paymentMethod: '',
    loyaltyPointsEarned: 0,
    completedAt: null,
    isCompleted: false,
  };

  if (!eventBook?.pages?.length) return state;

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('OrderCreated')) {
        const event = OrderCreated.decode(page.event.value) as any;
        state.customerId = event.customerId;
        state.items = event.items || [];
        state.subtotalCents = event.subtotalCents;
        state.discountCents = event.discountCents || 0;
      } else if (typeUrl.endsWith('LoyaltyDiscountApplied')) {
        const event = LoyaltyDiscountApplied.decode(page.event.value) as any;
        state.discountCents += event.discountCents;
        state.loyaltyPointsUsed = event.pointsUsed;
      } else if (typeUrl.endsWith('PaymentSubmitted')) {
        const event = PaymentSubmitted.decode(page.event.value) as any;
        state.paymentMethod = event.paymentMethod;
        state.finalTotalCents = event.amountCents;
      } else if (typeUrl.endsWith('OrderCompleted')) {
        const event = OrderCompleted.decode(page.event.value) as any;
        state.completedAt = event.completedAt;
        state.isCompleted = true;
        const pointsPerDollar = 10;
        state.loyaltyPointsEarned = Math.floor(state.finalTotalCents / 100) * pointsPerDollar;
      }
    } catch (e) {
      logger.warn({ err: e, typeUrl }, 'failed to decode event');
    }
  }

  return state;
}

const projectorService = {
  Handle(call: grpc.ServerUnaryCall<any, any>, callback: grpc.sendUnaryData<any>) {
    callback(null, {});
  },

  HandleSync(call: grpc.ServerUnaryCall<any, any>, callback: grpc.sendUnaryData<any>) {
    try {
      const eventBook = call.request;
      const state = buildReceiptState(eventBook);

      if (!state.isCompleted) {
        callback(null, null);
        return;
      }

      const orderId = uuidToHex(eventBook.cover?.root);

      const receipt = {
        orderId: orderId,
        customerId: state.customerId,
        items: state.items,
        subtotalCents: state.subtotalCents,
        discountCents: state.discountCents,
        finalTotalCents: state.finalTotalCents,
        paymentMethod: state.paymentMethod,
        loyaltyPointsEarned: state.loyaltyPointsEarned,
        completedAt: state.completedAt,
        formattedText: formatReceipt(state),
      };

      logger.info({ orderId, total: state.finalTotalCents }, 'generated receipt');

      const projection = {
        cover: eventBook.cover,
        projector: PROJECTOR_NAME,
        sequence: eventBook.pages?.length || 0,
        projection: encodeToAny(Receipt, receipt, 'examples.Receipt'),
      };

      callback(null, projection);
    } catch (err: any) {
      logger.error({ err }, 'projector error');
      callback({ code: grpc.status.INTERNAL, message: err.message || 'Internal error' });
    }
  },
};

async function main() {
  await loadProtoTypes();

  const port = process.env.PORT || '50410';
  const server = new grpc.Server();

  server.addService(grpcProto.angzarr.ProjectorCoordinator.service, projectorService);

  const healthImpl = new HealthImplementation({ '': 'SERVING' });
  healthImpl.addToServer(server);

  server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
    if (err) {
      logger.fatal({ err }, 'failed to bind server');
      process.exit(1);
    }
    logger.info({ projector: PROJECTOR_NAME, port: boundPort, listens_to: 'order domain' }, 'projector server started');
  });
}

main();
