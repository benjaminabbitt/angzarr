/**
 * Customer Service - TypeScript Implementation
 *
 * Handles customer lifecycle and loyalty points management.
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
const DOMAIN = 'customer';

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

// Load proto definitions for message encoding/decoding
let root: protobuf.Root;
let CustomerCreated: protobuf.Type;
let LoyaltyPointsAdded: protobuf.Type;
let LoyaltyPointsRedeemed: protobuf.Type;
let CustomerState: protobuf.Type;
let CreateCustomer: protobuf.Type;
let AddLoyaltyPoints: protobuf.Type;
let RedeemLoyaltyPoints: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  CustomerCreated = root.lookupType('examples.CustomerCreated');
  LoyaltyPointsAdded = root.lookupType('examples.LoyaltyPointsAdded');
  LoyaltyPointsRedeemed = root.lookupType('examples.LoyaltyPointsRedeemed');
  CustomerState = root.lookupType('examples.CustomerState');
  CreateCustomer = root.lookupType('examples.CreateCustomer');
  AddLoyaltyPoints = root.lookupType('examples.AddLoyaltyPoints');
  RedeemLoyaltyPoints = root.lookupType('examples.RedeemLoyaltyPoints');
}

// Customer state interface
interface ICustomerState {
  name: string;
  email: string;
  loyaltyPoints: number;
  lifetimePoints: number;
}

// Rebuild state from events
function rebuildState(eventBook: any): ICustomerState {
  const state: ICustomerState = {
    name: '',
    email: '',
    loyaltyPoints: 0,
    lifetimePoints: 0,
  };

  if (!eventBook?.pages?.length) {
    return state;
  }

  // Start from snapshot if present
  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = CustomerState.decode(eventBook.snapshot.state.value) as any;
      state.name = snapState.name || '';
      state.email = snapState.email || '';
      state.loyaltyPoints = snapState.loyaltyPoints || 0;
      state.lifetimePoints = snapState.lifetimePoints || 0;
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  // Apply events
  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('CustomerCreated')) {
        const event = CustomerCreated.decode(page.event.value) as any;
        state.name = event.name;
        state.email = event.email;
      } else if (typeUrl.endsWith('LoyaltyPointsAdded')) {
        const event = LoyaltyPointsAdded.decode(page.event.value) as any;
        state.loyaltyPoints = event.newBalance;
        state.lifetimePoints += event.points;
      } else if (typeUrl.endsWith('LoyaltyPointsRedeemed')) {
        const event = LoyaltyPointsRedeemed.decode(page.event.value) as any;
        state.loyaltyPoints = event.newBalance;
      }
    } catch (e) {
      logger.warn({ err: e, typeUrl }, 'failed to decode event');
    }
  }

  return state;
}

// Encode message to Any
function encodeToAny(messageType: protobuf.Type, message: any, typeName: string): any {
  const encoded = messageType.encode(messageType.create(message)).finish();
  return {
    type_url: `type.examples/${typeName}`,
    value: Buffer.from(encoded),
  };
}

// Get next sequence number
function nextSequence(priorEvents: any): number {
  if (!priorEvents?.pages?.length) return 0;
  return priorEvents.pages.length;
}

// Current timestamp
function now() {
  return { seconds: Math.floor(Date.now() / 1000), nanos: 0 };
}

// Handle CreateCustomer command
function handleCreateCustomer(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICustomerState,
  seq: number
): any {
  if (state.name) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Customer already exists',
    };
  }

  const cmd = CreateCustomer.decode(cmdData) as any;

  if (!cmd.name) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Customer name is required',
    };
  }
  if (!cmd.email) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Customer email is required',
    };
  }

  logger.info({ name: cmd.name, email: cmd.email }, 'creating customer');

  const event = {
    name: cmd.name,
    email: cmd.email,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(CustomerCreated, event, 'examples.CustomerCreated'),
          created_at: now(),
        },
      ],
    },
  };
}

// Handle AddLoyaltyPoints command
function handleAddLoyaltyPoints(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICustomerState,
  seq: number
): any {
  if (!state.name) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Customer does not exist',
    };
  }

  const cmd = AddLoyaltyPoints.decode(cmdData) as any;

  if (cmd.points <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Points must be positive',
    };
  }

  const newBalance = state.loyaltyPoints + cmd.points;

  logger.info(
    { points: cmd.points, new_balance: newBalance, reason: cmd.reason },
    'adding loyalty points'
  );

  const event = {
    points: cmd.points,
    newBalance: newBalance,
    reason: cmd.reason,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(LoyaltyPointsAdded, event, 'examples.LoyaltyPointsAdded'),
          created_at: now(),
        },
      ],
    },
  };
}

// Handle RedeemLoyaltyPoints command
function handleRedeemLoyaltyPoints(
  cmdBook: any,
  cmdData: Uint8Array,
  state: ICustomerState,
  seq: number
): any {
  if (!state.name) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Customer does not exist',
    };
  }

  const cmd = RedeemLoyaltyPoints.decode(cmdData) as any;

  if (cmd.points <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Points must be positive',
    };
  }
  if (cmd.points > state.loyaltyPoints) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: `Insufficient points: have ${state.loyaltyPoints}, need ${cmd.points}`,
    };
  }

  const newBalance = state.loyaltyPoints - cmd.points;

  logger.info(
    { points: cmd.points, new_balance: newBalance, redemption_type: cmd.redemptionType },
    'redeeming loyalty points'
  );

  const event = {
    points: cmd.points,
    newBalance: newBalance,
    redemptionType: cmd.redemptionType,
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(LoyaltyPointsRedeemed, event, 'examples.LoyaltyPointsRedeemed'),
          created_at: now(),
        },
      ],
    },
  };
}

// gRPC service implementation
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

      if (typeUrl.endsWith('CreateCustomer')) {
        response = handleCreateCustomer(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('AddLoyaltyPoints')) {
        response = handleAddLoyaltyPoints(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('RedeemLoyaltyPoints')) {
        response = handleRedeemLoyaltyPoints(cmdBook, cmdData, state, seq);
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

// Start server
async function main() {
  await loadProtoTypes();

  const port = process.env.PORT || '50052';
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
