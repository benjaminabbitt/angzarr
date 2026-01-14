import { Given, When, Then, Before } from '@cucumber/cucumber';
import { strict as assert } from 'assert';
import { LogProjectorLogic, EventPage, EventBook, LogEntry } from '../../src/log-projector-logic.js';

let logic: LogProjectorLogic;
let priorEvents: EventPage[];
let logEntries: LogEntry[];

Before(function () {
  logic = new LogProjectorLogic();
  priorEvents = [];
  logEntries = [];
});

// --- Given steps ---

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
  'a TransactionCreated event with customer {string} and subtotal {int}',
  function (customerId: string, subtotal: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCreated',
      data: { customerId, subtotalCents: subtotal },
    });
  }
);

Given(
  'a TransactionCompleted event with total {int} and payment {string}',
  function (total: number, paymentMethod: string) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCompleted',
      data: { finalTotalCents: total, paymentMethod },
    });
  }
);

Given('an unknown event type', function () {
  priorEvents.push({
    typeUrl: 'type.googleapis.com/examples.UnknownEvent',
    data: { foo: 'bar' },
  });
});

// --- When steps ---

When('I process the log projector', function () {
  const eventBook: EventBook = {
    cover: { domain: 'customer' },
    pages: priorEvents,
  };
  logEntries = logic.processEvents(eventBook);
});

// --- Then steps ---

Then('the event is logged successfully', function () {
  assert(logEntries.length > 0, 'Expected at least one log entry');
  // Successful logging means we got entries without unknown flag
  const lastEntry = logEntries[logEntries.length - 1];
  assert(lastEntry.fields.unknown !== true, 'Expected known event type');
});

Then('the event is logged as unknown', function () {
  assert(logEntries.length > 0, 'Expected at least one log entry');
  const lastEntry = logEntries[logEntries.length - 1];
  assert(lastEntry.fields.unknown === true, 'Expected unknown event type');
});
