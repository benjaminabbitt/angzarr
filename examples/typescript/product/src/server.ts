/**
 * Product Service - TypeScript Implementation
 *
 * Handles product catalog management.
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
const DOMAIN = 'product';

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
let ProductCreated: protobuf.Type;
let ProductUpdated: protobuf.Type;
let PriceSet: protobuf.Type;
let ProductDiscontinued: protobuf.Type;
let ProductState: protobuf.Type;
let CreateProduct: protobuf.Type;
let UpdateProduct: protobuf.Type;
let SetPrice: protobuf.Type;
let Discontinue: protobuf.Type;

async function loadProtoTypes() {
  root = await protobuf.load([
    path.join(PROTO_PATH, 'angzarr/angzarr.proto'),
    path.join(PROTO_PATH, 'examples/domains.proto'),
  ]);

  ProductCreated = root.lookupType('examples.ProductCreated');
  ProductUpdated = root.lookupType('examples.ProductUpdated');
  PriceSet = root.lookupType('examples.PriceSet');
  ProductDiscontinued = root.lookupType('examples.ProductDiscontinued');
  ProductState = root.lookupType('examples.ProductState');
  CreateProduct = root.lookupType('examples.CreateProduct');
  UpdateProduct = root.lookupType('examples.UpdateProduct');
  SetPrice = root.lookupType('examples.SetPrice');
  Discontinue = root.lookupType('examples.Discontinue');
}

interface IProductState {
  sku: string;
  name: string;
  description: string;
  priceCents: number;
  status: string;
}

function emptyState(): IProductState {
  return {
    sku: '',
    name: '',
    description: '',
    priceCents: 0,
    status: '',
  };
}

function rebuildState(eventBook: any): IProductState {
  const state = emptyState();

  if (!eventBook?.pages?.length) {
    return state;
  }

  if (eventBook.snapshot?.state?.value) {
    try {
      const snapState = ProductState.decode(eventBook.snapshot.state.value) as any;
      state.sku = snapState.sku || '';
      state.name = snapState.name || '';
      state.description = snapState.description || '';
      state.priceCents = snapState.priceCents || 0;
      state.status = snapState.status || '';
    } catch (e) {
      logger.warn({ err: e }, 'failed to decode snapshot');
    }
  }

  for (const page of eventBook.pages) {
    if (!page.event?.value) continue;

    const typeUrl = page.event.type_url || '';

    try {
      if (typeUrl.endsWith('ProductCreated')) {
        const event = ProductCreated.decode(page.event.value) as any;
        state.sku = event.sku;
        state.name = event.name;
        state.description = event.description;
        state.priceCents = event.priceCents;
        state.status = 'active';
      } else if (typeUrl.endsWith('ProductUpdated')) {
        const event = ProductUpdated.decode(page.event.value) as any;
        state.name = event.name;
        state.description = event.description;
      } else if (typeUrl.endsWith('PriceSet')) {
        const event = PriceSet.decode(page.event.value) as any;
        state.priceCents = event.newPriceCents;
      } else if (typeUrl.endsWith('ProductDiscontinued')) {
        state.status = 'discontinued';
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

function handleCreateProduct(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IProductState,
  seq: number
): any {
  if (state.sku) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Product already exists',
    };
  }

  const cmd = CreateProduct.decode(cmdData) as any;

  if (!cmd.sku) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product SKU is required',
    };
  }
  if (!cmd.name) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product name is required',
    };
  }
  if (cmd.priceCents <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Price must be positive',
    };
  }

  logger.info({ sku: cmd.sku, name: cmd.name }, 'creating product');

  const event = {
    sku: cmd.sku,
    name: cmd.name,
    description: cmd.description || '',
    priceCents: cmd.priceCents,
    createdAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ProductCreated, event, 'examples.ProductCreated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleUpdateProduct(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IProductState,
  seq: number
): any {
  if (!state.sku) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Product does not exist',
    };
  }
  if (state.status === 'discontinued') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot update discontinued product',
    };
  }

  const cmd = UpdateProduct.decode(cmdData) as any;

  if (!cmd.name) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Product name is required',
    };
  }

  logger.info({ sku: state.sku, name: cmd.name }, 'updating product');

  const event = {
    name: cmd.name,
    description: cmd.description || '',
    updatedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ProductUpdated, event, 'examples.ProductUpdated'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleSetPrice(
  cmdBook: any,
  cmdData: Uint8Array,
  state: IProductState,
  seq: number
): any {
  if (!state.sku) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Product does not exist',
    };
  }
  if (state.status === 'discontinued') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Cannot set price on discontinued product',
    };
  }

  const cmd = SetPrice.decode(cmdData) as any;

  if (cmd.newPriceCents <= 0) {
    throw {
      code: grpc.status.INVALID_ARGUMENT,
      message: 'Price must be positive',
    };
  }

  logger.info({ sku: state.sku, old_price: state.priceCents, new_price: cmd.newPriceCents }, 'setting price');

  const event = {
    oldPriceCents: state.priceCents,
    newPriceCents: cmd.newPriceCents,
    effectiveAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(PriceSet, event, 'examples.PriceSet'),
          created_at: now(),
        },
      ],
    },
  };
}

function handleDiscontinue(
  cmdBook: any,
  _cmdData: Uint8Array,
  state: IProductState,
  seq: number
): any {
  if (!state.sku) {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Product does not exist',
    };
  }
  if (state.status === 'discontinued') {
    throw {
      code: grpc.status.FAILED_PRECONDITION,
      message: 'Product already discontinued',
    };
  }

  logger.info({ sku: state.sku }, 'discontinuing product');

  const event = {
    discontinuedAt: now(),
  };

  return {
    events: {
      cover: cmdBook.cover,
      pages: [
        {
          num: seq,
          event: encodeToAny(ProductDiscontinued, event, 'examples.ProductDiscontinued'),
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

      if (typeUrl.endsWith('CreateProduct')) {
        response = handleCreateProduct(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('UpdateProduct')) {
        response = handleUpdateProduct(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('SetPrice')) {
        response = handleSetPrice(cmdBook, cmdData, state, seq);
      } else if (typeUrl.endsWith('Discontinue')) {
        response = handleDiscontinue(cmdBook, cmdData, state, seq);
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

  const port = process.env.PORT || '50401';
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
