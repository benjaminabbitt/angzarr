/**
 * Loyalty Saga Service - TypeScript Implementation
 *
 * Listens to TransactionCompleted events and generates AddLoyaltyPoints commands.
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
const SAGA_NAME = 'loyalty';
const packageDefinition = protoLoader.loadSync([path.join(PROTO_PATH, 'angzarr/angzarr.proto')], {
    keepCase: true,
    longs: String,
    enums: String,
    defaults: true,
    oneofs: true,
    includeDirs: [PROTO_PATH],
});
const grpcProto = grpc.loadPackageDefinition(packageDefinition);
let TransactionCompleted;
let AddLoyaltyPoints;
async function loadProtoTypes() {
    const root = await protobuf.load([
        path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
        path.join(PROTO_PATH, 'examples/domains.proto'),
    ]);
    TransactionCompleted = root.lookupType('examples.TransactionCompleted');
    AddLoyaltyPoints = root.lookupType('examples.AddLoyaltyPoints');
}
function encodeToAny(messageType, message, typeName) {
    const encoded = messageType.encode(messageType.create(message)).finish();
    return {
        type_url: `type.examples/${typeName}`,
        value: Buffer.from(encoded),
    };
}
// Convert UUID bytes to hex string
function uuidToHex(uuid) {
    if (!uuid?.value)
        return '';
    const buf = Buffer.from(uuid.value);
    return buf.toString('hex');
}
const sagaService = {
    Handle(call, callback) {
        // Async handler - fire and forget
        callback(null, {});
    },
    HandleSync(call, callback) {
        try {
            const eventBook = call.request;
            const commands = [];
            if (!eventBook?.pages?.length) {
                callback(null, { commands: [] });
                return;
            }
            const customerId = eventBook.cover?.root;
            const transactionId = uuidToHex(customerId);
            for (const page of eventBook.pages) {
                if (!page.event?.value)
                    continue;
                const typeUrl = page.event.type_url || '';
                if (typeUrl.endsWith('TransactionCompleted')) {
                    try {
                        const event = TransactionCompleted.decode(page.event.value);
                        const points = event.loyaltyPointsEarned || 0;
                        if (points > 0) {
                            logger.info({ transactionId, points, saga: SAGA_NAME }, 'generating AddLoyaltyPoints command');
                            // Generate command for customer aggregate
                            // Note: The customer_id from TransactionCreated would be needed
                            // For this demo, we use a placeholder approach
                            const command = {
                                points: points,
                                reason: `transaction:${transactionId}`,
                            };
                            commands.push({
                                cover: {
                                    domain: 'customer',
                                    // In real implementation, we'd get customer_id from prior events
                                    root: customerId,
                                },
                                pages: [
                                    {
                                        command: encodeToAny(AddLoyaltyPoints, command, 'examples.AddLoyaltyPoints'),
                                    },
                                ],
                            });
                        }
                    }
                    catch (e) {
                        logger.warn({ err: e }, 'failed to decode TransactionCompleted');
                    }
                }
            }
            callback(null, { commands });
        }
        catch (err) {
            logger.error({ err }, 'saga error');
            callback({ code: grpc.status.INTERNAL, message: err.message || 'Internal error' });
        }
    },
};
async function main() {
    await loadProtoTypes();
    const port = process.env.PORT || '50054';
    const server = new grpc.Server();
    server.addService(grpcProto.angzarr.Saga.service, sagaService);
    const healthImpl = new HealthImplementation({ '': 'SERVING' });
    healthImpl.addToServer(server);
    server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
        if (err) {
            logger.fatal({ err }, 'failed to bind server');
            process.exit(1);
        }
        logger.info({ saga: SAGA_NAME, port: boundPort }, 'saga server started');
    });
}
main();
//# sourceMappingURL=server.js.map