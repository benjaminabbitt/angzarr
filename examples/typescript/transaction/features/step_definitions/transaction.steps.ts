import { Given, When, Then, Before, DataTable } from '@cucumber/cucumber';
import { strict as assert } from 'assert';
import {
  TransactionLogic,
  TransactionCreatedEvent,
  DiscountAppliedEvent,
  TransactionCompletedEvent,
  TransactionCancelledEvent,
  EventPage,
  EventBook,
} from '../../src/transaction-logic.js';
import { CommandValidationError, StatusCode } from '../../src/command-validation-error.js';
import { TransactionState, LineItem } from '../../src/transaction-state.js';

let logic: TransactionLogic;
let priorEvents: EventPage[];
let resultEvent:
  | TransactionCreatedEvent
  | DiscountAppliedEvent
  | TransactionCompletedEvent
  | TransactionCancelledEvent
  | null;
let error: CommandValidationError | null;
let state: TransactionState | null;

Before(function () {
  logic = new TransactionLogic();
  priorEvents = [];
  resultEvent = null;
  error = null;
  state = null;
});

// --- Given steps ---

Given('no prior events for the aggregate', function () {
  priorEvents = [];
});

Given(
  'a TransactionCreated event with customer {string} and subtotal {int}',
  function (customerId: string, subtotal: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCreated',
      data: { customerId, items: [], subtotalCents: subtotal },
    });
  }
);

Given(
  'a TransactionCreated event with customer {string} and items:',
  function (customerId: string, dataTable: DataTable) {
    const items = dataTable.hashes().map((row) => ({
      productId: row.product_id,
      name: row.name,
      quantity: parseInt(row.quantity, 10),
      unitPriceCents: parseInt(row.unit_price_cents, 10),
    }));
    const subtotalCents = items.reduce(
      (sum, item) => sum + item.quantity * item.unitPriceCents,
      0
    );
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCreated',
      data: { customerId, items, subtotalCents },
    });
  }
);

Given('a TransactionCompleted event', function () {
  priorEvents.push({
    typeUrl: 'type.googleapis.com/examples.TransactionCompleted',
    data: { finalTotalCents: 0 },
  });
});

Given('a DiscountApplied event with {int} cents discount', function (discountCents: number) {
  priorEvents.push({
    typeUrl: 'type.googleapis.com/examples.DiscountApplied',
    data: { discountType: 'fixed', discountCents },
  });
});

// --- When steps ---

When(
  'I handle a CreateTransaction command with customer {string} and items:',
  function (customerId: string, dataTable: DataTable) {
    const items: LineItem[] = dataTable.hashes().map((row) => ({
      productId: row.product_id,
      name: row.name,
      quantity: parseInt(row.quantity, 10),
      unitPriceCents: parseInt(row.unit_price_cents, 10),
    }));
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleCreateTransaction(state, customerId, items);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle a CreateTransaction command with customer {string} and no items',
  function (customerId: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleCreateTransaction(state, customerId, []);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle an ApplyDiscount command with type {string} and value {int}',
  function (discountType: string, value: number) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleApplyDiscount(state, discountType, value);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle a CompleteTransaction command with payment method {string}',
  function (paymentMethod: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleCompleteTransaction(state, paymentMethod);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle a CancelTransaction command with reason {string}',
  function (reason: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleCancelTransaction(state, reason);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When('I rebuild the transaction state', function () {
  const eventBook = buildEventBook();
  state = logic.rebuildState(eventBook);
});

// --- Then steps ---

Then('the result is a TransactionCreated event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('customerId' in resultEvent && 'subtotalCents' in resultEvent);
});

Then('the result is a DiscountApplied event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('discountType' in resultEvent && 'discountCents' in resultEvent);
});

Then('the result is a TransactionCompleted event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('finalTotalCents' in resultEvent && 'paymentMethod' in resultEvent);
});

Then('the result is a TransactionCancelled event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('reason' in resultEvent && 'cancelledAt' in resultEvent);
});

Then('the command fails with status {string}', function (statusName: string) {
  assert(error !== null, 'Expected command to fail but it succeeded');
  const expectedCode = StatusCode[statusName as keyof typeof StatusCode];
  assert.strictEqual(error.statusCode, expectedCode, `Expected status ${statusName}`);
});

Then('the event has customer_id {string}', function (customerId: string) {
  assert(resultEvent !== null);
  assert('customerId' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCreatedEvent).customerId, customerId);
});

Then('the event has subtotal_cents {int}', function (subtotalCents: number) {
  assert(resultEvent !== null);
  assert('subtotalCents' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCreatedEvent).subtotalCents, subtotalCents);
});

Then('the event has discount_cents {int}', function (discountCents: number) {
  assert(resultEvent !== null);
  assert('discountCents' in resultEvent);
  assert.strictEqual((resultEvent as DiscountAppliedEvent).discountCents, discountCents);
});

Then('the event has final_total_cents {int}', function (finalTotalCents: number) {
  assert(resultEvent !== null);
  assert('finalTotalCents' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCompletedEvent).finalTotalCents, finalTotalCents);
});

Then('the event has payment_method {string}', function (paymentMethod: string) {
  assert(resultEvent !== null);
  assert('paymentMethod' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCompletedEvent).paymentMethod, paymentMethod);
});

Then('the event has loyalty_points_earned {int}', function (points: number) {
  assert(resultEvent !== null);
  assert('loyaltyPointsEarned' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCompletedEvent).loyaltyPointsEarned, points);
});

Then('the event has reason {string}', function (reason: string) {
  assert(resultEvent !== null);
  assert('reason' in resultEvent);
  assert.strictEqual((resultEvent as TransactionCancelledEvent).reason, reason);
});

Then('the state has customer_id {string}', function (customerId: string) {
  assert(state !== null);
  assert.strictEqual(state.customerId, customerId);
});

Then('the state has subtotal_cents {int}', function (subtotalCents: number) {
  assert(state !== null);
  assert.strictEqual(state.subtotalCents, subtotalCents);
});

Then('the state has status {string}', function (status: string) {
  assert(state !== null);
  assert.strictEqual(state.status, status);
});

// --- Helpers ---

function buildEventBook(): EventBook | null {
  if (priorEvents.length === 0) {
    return null;
  }
  return { pages: priorEvents };
}
