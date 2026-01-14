import { Given, When, Then, Before } from '@cucumber/cucumber';
import { strict as assert } from 'assert';
import { LoyaltySagaLogic, EventPage, EventBook, AddLoyaltyPointsCommand } from '../../src/loyalty-saga-logic.js';

let logic: LoyaltySagaLogic;
let priorEvents: EventPage[];
let commands: AddLoyaltyPointsCommand[];

Before(function () {
  logic = new LoyaltySagaLogic();
  priorEvents = [];
  commands = [];
});

// --- Given steps ---

Given(
  'a TransactionCreated event with customer {string} and subtotal {int}',
  function (customerId: string, subtotal: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCreated',
      data: { customerId, subtotalCents: subtotal },
    });
  }
);

Given(
  'a TransactionCompleted event with {int} loyalty points earned',
  function (points: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCompleted',
      data: { loyaltyPointsEarned: points },
    });
  }
);

// --- When steps ---

When('I process the saga', function () {
  const eventBook: EventBook = {
    cover: {
      domain: 'transaction',
      root: { value: Buffer.from('test-transaction-id') },
    },
    pages: priorEvents,
  };
  commands = logic.process(eventBook);
});

// --- Then steps ---

Then('no commands are generated', function () {
  assert.strictEqual(commands.length, 0, 'Expected no commands to be generated');
});

Then('an AddLoyaltyPoints command is generated', function () {
  assert(commands.length > 0, 'Expected at least one command to be generated');
});

Then('the command has points {int}', function (points: number) {
  assert(commands.length > 0);
  assert.strictEqual(commands[0].points, points);
});

Then('the command has domain {string}', function (domain: string) {
  assert(commands.length > 0);
  assert.strictEqual(commands[0].domain, domain);
});

Then('the command reason contains {string}', function (substring: string) {
  assert(commands.length > 0);
  assert(
    commands[0].reason.toLowerCase().includes(substring.toLowerCase()),
    `Expected reason to contain '${substring}' but was '${commands[0].reason}'`
  );
});
