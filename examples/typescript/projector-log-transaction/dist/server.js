/**
 * Transaction Log Projector - TypeScript Implementation
 *
 * Logs transaction events using structured logging.
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
const logger = pino({ level: 'info' });
const PROJECTOR_NAME = 'log-transaction';
const packageDefinition = protoLoader.loadSync([path.join(PROTO_PATH, 'angzarr/angzarr.proto')], {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
    includeDirs: [PROTO_PATH],
});
const grpcProto = grpc.loadPackageDefinition(packageDefinition);
let TransactionCreated;
let DiscountApplied;
let TransactionCompleted;
let TransactionCancelled;
async function loadProtoTypes() {
    const root = await protobuf.load([
        path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
        path.join(PROTO_PATH, 'examples/domains.proto'),
    ]);
    TransactionCreated = root.lookupType('examples.TransactionCreated');
    DiscountApplied = root.lookupType('examples.DiscountApplied');
    TransactionCompleted = root.lookupType('examples.TransactionCompleted');
    TransactionCancelled = root.lookupType('examples.TransactionCancelled');
}
function uuidToHex(uuid) {
    if (!uuid?.value)
        return '';
    return Buffer.from(uuid.value).toString('hex');
}
function logEvents(eventBook) {
    if (!eventBook?.pages?.length)
        return;
    const domain = eventBook.cover?.domain || 'transaction';
    const rootId = uuidToHex(eventBook.cover?.root);
    const shortId = rootId.slice(0, 16);
    for (const page of eventBook.pages) {
        if (!page.event?.value)
            continue;
        const typeUrl = page.event.type_url || '';
        const eventType = typeUrl.split('.').pop() || typeUrl;
        const sequence = page.num || 0;
        const baseLog = { domain, root_id: shortId, sequence, event_type: eventType };
        try {
            if (typeUrl.endsWith('TransactionCreated')) {
                const event = TransactionCreated.decode(page.event.value);
                logger.info({ ...baseLog, customer_id: event.customerId, item_count: event.items?.length || 0, subtotal: event.subtotalCents }, 'event');
            }
            else if (typeUrl.endsWith('DiscountApplied')) {
                const event = DiscountApplied.decode(page.event.value);
                logger.info({ ...baseLog, discount_type: event.discountType, value: event.value, discount_cents: event.discountCents }, 'event');
            }
            else if (typeUrl.endsWith('TransactionCompleted')) {
                const event = TransactionCompleted.decode(page.event.value);
                logger.info({ ...baseLog, final_total: event.finalTotalCents, payment_method: event.paymentMethod, loyalty_points: event.loyaltyPointsEarned }, 'event');
            }
            else if (typeUrl.endsWith('TransactionCancelled')) {
                const event = TransactionCancelled.decode(page.event.value);
                logger.info({ ...baseLog, reason: event.reason }, 'event');
            }
            else {
                logger.info({ ...baseLog, raw_bytes: page.event.value.length }, 'event');
            }
        }
        catch (e) {
            logger.warn({ err: e, typeUrl }, 'failed to decode event');
        }
    }
}
const projectorService = {
    Handle(call, callback) {
        logEvents(call.request);
        callback(null, {});
    },
    HandleSync(call, callback) {
        logEvents(call.request);
        // Log projector doesn't produce a projection
        callback(null, null);
    },
};
async function main() {
    await loadProtoTypes();
    const port = process.env.PORT || '50057';
    const server = new grpc.Server();
    server.addService(grpcProto.angzarr.ProjectorCoordinator.service, projectorService);
    const healthImpl = new HealthImplementation({ '': 'SERVING' });
    healthImpl.addToServer(server);
    server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
        if (err) {
            logger.fatal({ err }, 'failed to bind server');
            process.exit(1);
        }
        logger.info({ projector: PROJECTOR_NAME, port: boundPort, listens_to: 'transaction domain' }, 'projector server started');
    });
}
main();
//# sourceMappingURL=server.js.map