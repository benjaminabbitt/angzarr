import { Given, When, Then, Before, DataTable } from '@cucumber/cucumber';
import { strict as assert } from 'assert';
import { ReceiptProjectorLogic, EventPage, EventBook, Receipt } from '../../src/receipt-projector-logic.js';

let logic: ReceiptProjectorLogic;
let priorEvents: EventPage[];
let receipt: Receipt | null;

Before(function () {
  logic = new ReceiptProjectorLogic();
  priorEvents = [];
  receipt = null;
});

// --- Given steps ---

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

Given('a DiscountApplied event with {int} cents discount', function (discountCents: number) {
  priorEvents.push({
    typeUrl: 'type.googleapis.com/examples.DiscountApplied',
    data: { discountCents },
  });
});

Given(
  'a TransactionCompleted event with total {int} and payment {string}',
  function (total: number, paymentMethod: string) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCompleted',
      data: { finalTotalCents: total, paymentMethod, loyaltyPointsEarned: 0 },
    });
  }
);

Given(
  'a TransactionCompleted event with total {int} and payment {string} earning {int} points',
  function (total: number, paymentMethod: string, points: number) {
    priorEvents.push({
      typeUrl: 'type.googleapis.com/examples.TransactionCompleted',
      data: { finalTotalCents: total, paymentMethod, loyaltyPointsEarned: points },
    });
  }
);

// --- When steps ---

When('I project the events', function () {
  const eventBook: EventBook = {
    cover: { domain: 'transaction' },
    pages: priorEvents,
  };
  receipt = logic.project(eventBook);
});

// --- Then steps ---

Then('no projection is generated', function () {
  assert.strictEqual(receipt, null, 'Expected no projection');
});

Then('a Receipt projection is generated', function () {
  assert(receipt !== null, 'Expected a receipt projection');
});

Then('the receipt has customer_id {string}', function (customerId: string) {
  assert(receipt !== null);
  assert.strictEqual(receipt.customerId, customerId);
});

Then('the receipt has subtotal_cents {int}', function (subtotalCents: number) {
  assert(receipt !== null);
  assert.strictEqual(receipt.subtotalCents, subtotalCents);
});

Then('the receipt has discount_cents {int}', function (discountCents: number) {
  assert(receipt !== null);
  assert.strictEqual(receipt.discountCents, discountCents);
});

Then('the receipt has final_total_cents {int}', function (finalTotalCents: number) {
  assert(receipt !== null);
  assert.strictEqual(receipt.finalTotalCents, finalTotalCents);
});

Then('the receipt has payment_method {string}', function (paymentMethod: string) {
  assert(receipt !== null);
  assert.strictEqual(receipt.paymentMethod, paymentMethod);
});

Then('the receipt has loyalty_points_earned {int}', function (points: number) {
  assert(receipt !== null);
  assert.strictEqual(receipt.loyaltyPointsEarned, points);
});

Then('the receipt formatted_text contains {string}', function (substring: string) {
  assert(receipt !== null);
  assert(
    receipt.formattedText.includes(substring),
    `Expected formatted_text to contain '${substring}' but was:\n${receipt.formattedText}`
  );
});
