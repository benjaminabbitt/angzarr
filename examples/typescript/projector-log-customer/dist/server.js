/**
 * Customer Log Projector - TypeScript Implementation
 *
 * Logs customer events using structured logging.
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
const PROJECTOR_NAME = 'log-customer';
const packageDefinition = protoLoader.loadSync([path.join(PROTO_PATH, 'angzarr/angzarr.proto')], {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
    includeDirs: [PROTO_PATH],
});
const grpcProto = grpc.loadPackageDefinition(packageDefinition);
let CustomerCreated;
let LoyaltyPointsAdded;
let LoyaltyPointsRedeemed;
async function loadProtoTypes() {
    const root = await protobuf.load([
        path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
        path.join(PROTO_PATH, 'examples/domains.proto'),
    ]);
    CustomerCreated = root.lookupType('examples.CustomerCreated');
    LoyaltyPointsAdded = root.lookupType('examples.LoyaltyPointsAdded');
    LoyaltyPointsRedeemed = root.lookupType('examples.LoyaltyPointsRedeemed');
}
function uuidToHex(uuid) {
    if (!uuid?.value)
        return '';
    return Buffer.from(uuid.value).toString('hex');
}
function logEvents(eventBook) {
    if (!eventBook?.pages?.length)
        return;
    const domain = eventBook.cover?.domain || 'customer';
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
            if (typeUrl.endsWith('CustomerCreated')) {
                const event = CustomerCreated.decode(page.event.value);
                logger.info({ ...baseLog, name: event.name, email: event.email }, 'event');
            }
            else if (typeUrl.endsWith('LoyaltyPointsAdded')) {
                const event = LoyaltyPointsAdded.decode(page.event.value);
                logger.info({ ...baseLog, points: event.points, new_balance: event.newBalance, reason: event.reason }, 'event');
            }
            else if (typeUrl.endsWith('LoyaltyPointsRedeemed')) {
                const event = LoyaltyPointsRedeemed.decode(page.event.value);
                logger.info({ ...baseLog, points: event.points, new_balance: event.newBalance, redemption_type: event.redemptionType }, 'event');
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
    const port = process.env.PORT || '50056';
    const server = new grpc.Server();
    server.addService(grpcProto.angzarr.ProjectorCoordinator.service, projectorService);
    const healthImpl = new HealthImplementation({ '': 'SERVING' });
    healthImpl.addToServer(server);
    server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
        if (err) {
            logger.fatal({ err }, 'failed to bind server');
            process.exit(1);
        }
        logger.info({ projector: PROJECTOR_NAME, port: boundPort, listens_to: 'customer domain' }, 'projector server started');
    });
}
main();
//# sourceMappingURL=server.js.map