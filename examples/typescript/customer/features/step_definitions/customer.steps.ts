import { Given, When, Then, Before } from '@cucumber/cucumber';
import { strict as assert } from 'assert';
import {
  CustomerLogic,
  CustomerCreatedEvent,
  LoyaltyPointsAddedEvent,
  LoyaltyPointsRedeemedEvent,
  EventPage,
  EventBook,
} from '../../src/customer-logic.js';
import { CommandValidationError, StatusCode } from '../../src/command-validation-error.js';
import { CustomerState } from '../../src/customer-state.js';

let logic: CustomerLogic;
let priorEvents: EventPage[];
let resultEvent:
  | CustomerCreatedEvent
  | LoyaltyPointsAddedEvent
  | LoyaltyPointsRedeemedEvent
  | null;
let error: CommandValidationError | null;
let state: CustomerState | null;

Before(function () {
  logic = new CustomerLogic();
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
  'a CustomerCreated event with name {string} and email {string}',
  function (name: string, email: string) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.CustomerCreated',
      data: { name, email },
    });
  }
);

Given(
  'a LoyaltyPointsAdded event with {int} points and new_balance {int}',
  function (points: number, newBalance: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.LoyaltyPointsAdded',
      data: { points, newBalance },
    });
  }
);

Given(
  'a LoyaltyPointsRedeemed event with {int} points and new_balance {int}',
  function (points: number, newBalance: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.LoyaltyPointsRedeemed',
      data: { points, newBalance },
    });
  }
);

// --- When steps ---

When(
  'I handle a CreateCustomer command with name {string} and email {string}',
  function (name: string, email: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleCreateCustomer(state, name, email);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle an AddLoyaltyPoints command with {int} points and reason {string}',
  function (points: number, reason: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleAddLoyaltyPoints(state, points, reason);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When(
  'I handle a RedeemLoyaltyPoints command with {int} points and type {string}',
  function (points: number, redemptionType: string) {
    const eventBook = buildEventBook();
    state = logic.rebuildState(eventBook);
    try {
      resultEvent = logic.handleRedeemLoyaltyPoints(state, points, redemptionType);
      error = null;
    } catch (e) {
      error = e as CommandValidationError;
      resultEvent = null;
    }
  }
);

When('I rebuild the customer state', function () {
  const eventBook = buildEventBook();
  state = logic.rebuildState(eventBook);
});

// --- Then steps ---

Then('the result is a CustomerCreated event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('name' in resultEvent && 'email' in resultEvent);
});

Then('the result is a LoyaltyPointsAdded event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert('points' in resultEvent && 'newBalance' in resultEvent && 'reason' in resultEvent);
});

Then('the result is a LoyaltyPointsRedeemed event', function () {
  assert(resultEvent !== null, `Expected result but got error: ${error?.message}`);
  assert(
    'points' in resultEvent &&
      'newBalance' in resultEvent &&
      'redemptionType' in resultEvent
  );
});

Then('the command fails with status {string}', function (statusName: string) {
  assert(error !== null, 'Expected command to fail but it succeeded');
  const expectedCode = StatusCode[statusName as keyof typeof StatusCode];
  assert.strictEqual(error.statusCode, expectedCode, `Expected status ${statusName}`);
});

Then('the error message contains {string}', function (substring: string) {
  assert(error !== null, 'Expected error but command succeeded');
  assert(
    error.message.toLowerCase().includes(substring.toLowerCase()),
    `Expected error message to contain '${substring}' but was '${error.message}'`
  );
});

Then('the event has name {string}', function (name: string) {
  assert(resultEvent !== null);
  assert('name' in resultEvent);
  assert.strictEqual((resultEvent as CustomerCreatedEvent).name, name);
});

Then('the event has email {string}', function (email: string) {
  assert(resultEvent !== null);
  assert('email' in resultEvent);
  assert.strictEqual((resultEvent as CustomerCreatedEvent).email, email);
});

Then('the event has points {int}', function (points: number) {
  assert(resultEvent !== null);
  assert('points' in resultEvent);
  assert.strictEqual(
    (resultEvent as LoyaltyPointsAddedEvent | LoyaltyPointsRedeemedEvent).points,
    points
  );
});

Then('the event has new_balance {int}', function (newBalance: number) {
  assert(resultEvent !== null);
  assert('newBalance' in resultEvent);
  assert.strictEqual(
    (resultEvent as LoyaltyPointsAddedEvent | LoyaltyPointsRedeemedEvent).newBalance,
    newBalance
  );
});

Then('the event has reason {string}', function (reason: string) {
  assert(resultEvent !== null);
  assert('reason' in resultEvent);
  assert.strictEqual((resultEvent as LoyaltyPointsAddedEvent).reason, reason);
});

Then('the event has redemption_type {string}', function (redemptionType: string) {
  assert(resultEvent !== null);
  assert('redemptionType' in resultEvent);
  assert.strictEqual(
    (resultEvent as LoyaltyPointsRedeemedEvent).redemptionType,
    redemptionType
  );
});

Then('the state has name {string}', function (name: string) {
  assert(state !== null);
  assert.strictEqual(state.name, name);
});

Then('the state has email {string}', function (email: string) {
  assert(state !== null);
  assert.strictEqual(state.email, email);
});

Then('the state has loyalty_points {int}', function (points: number) {
  assert(state !== null);
  assert.strictEqual(state.loyaltyPoints, points);
});

Then('the state has lifetime_points {int}', function (points: number) {
  assert(state !== null);
  assert.strictEqual(state.lifetimePoints, points);
});

// --- Helpers ---

function buildEventBook(): EventBook | null {
  if (priorEvents.length === 0) {
    return null;
  }
  return { pages: priorEvents };
}
