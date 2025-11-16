/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Simple test for C reference list implementation
 *
 * This verifies that our standalone C code works correctly
 * before using it as a reference in Rust tests.
 */

#include "list.h"
#include <stdio.h>
#include <assert.h>
#include <string.h>

/* Test data structure */
struct test_entry {
	int value;
	struct list_head list;
};

/* Test: Basic initialization */
static void test_init(void)
{
	struct list_head head;

	INIT_LIST_HEAD(&head);

	assert(head.next == &head);
	assert(head.prev == &head);
	assert(list_empty(&head));

	printf("✓ test_init passed\n");
}

/* Test: Add entries */
static void test_add(void)
{
	struct list_head head;
	struct list_head a, b, c;

	INIT_LIST_HEAD(&head);

	list_add(&a, &head);
	list_add(&b, &head);
	list_add(&c, &head);

	/* Should be: head -> c -> b -> a -> head */
	assert(head.next == &c);
	assert(c.next == &b);
	assert(b.next == &a);
	assert(a.next == &head);

	printf("✓ test_add passed\n");
}

/* Test: Add tail */
static void test_add_tail(void)
{
	struct list_head head;
	struct list_head a, b, c;

	INIT_LIST_HEAD(&head);

	list_add_tail(&a, &head);
	list_add_tail(&b, &head);
	list_add_tail(&c, &head);

	/* Should be: head -> a -> b -> c -> head */
	assert(head.next == &a);
	assert(a.next == &b);
	assert(b.next == &c);
	assert(c.next == &head);

	printf("✓ test_add_tail passed\n");
}

/* Test: Delete entry */
static void test_del(void)
{
	struct list_head head;
	struct list_head a, b, c;

	INIT_LIST_HEAD(&head);
	list_add_tail(&a, &head);
	list_add_tail(&b, &head);
	list_add_tail(&c, &head);

	/* Delete middle entry */
	list_del(&b);

	/* Should be: head -> a -> c -> head */
	assert(head.next == &a);
	assert(a.next == &c);
	assert(c.next == &head);
	assert(!list_empty(&head));

	printf("✓ test_del passed\n");
}

/* Test: Empty list */
static void test_empty(void)
{
	struct list_head head;
	struct list_head a;

	INIT_LIST_HEAD(&head);
	assert(list_empty(&head));

	list_add(&a, &head);
	assert(!list_empty(&head));

	list_del(&a);
	assert(list_empty(&head));

	printf("✓ test_empty passed\n");
}

/* Test: is_head, is_first, is_last */
static void test_position(void)
{
	struct list_head head;
	struct list_head a, b, c;

	INIT_LIST_HEAD(&head);
	list_add_tail(&a, &head);
	list_add_tail(&b, &head);
	list_add_tail(&c, &head);

	/* head -> a -> b -> c -> head */

	assert(list_is_head(&head, &head));
	assert(!list_is_head(&a, &head));

	assert(list_is_first(&a, &head));
	assert(!list_is_first(&b, &head));
	assert(!list_is_first(&c, &head));

	assert(list_is_last(&c, &head));
	assert(!list_is_last(&a, &head));
	assert(!list_is_last(&b, &head));

	printf("✓ test_position passed\n");
}

/* Test: structure layout */
static void test_layout(void)
{
	extern const size_t C_LIST_HEAD_SIZE;
	extern const size_t C_LIST_HEAD_ALIGN;
	extern const size_t C_LIST_HEAD_NEXT_OFFSET;
	extern const size_t C_LIST_HEAD_PREV_OFFSET;

	printf("  list_head size: %zu bytes\n", C_LIST_HEAD_SIZE);
	printf("  list_head align: %zu bytes\n", C_LIST_HEAD_ALIGN);
	printf("  next offset: %zu\n", C_LIST_HEAD_NEXT_OFFSET);
	printf("  prev offset: %zu\n", C_LIST_HEAD_PREV_OFFSET);

	/* Verify expected values for x86-64/ARM64 */
	assert(C_LIST_HEAD_SIZE == 16);  /* two pointers */
	assert(C_LIST_HEAD_NEXT_OFFSET == 0);
	assert(C_LIST_HEAD_PREV_OFFSET == 8);

	printf("✓ test_layout passed\n");
}

int main(void)
{
	printf("Running C reference list tests...\n");
	printf("==================================\n\n");

	test_init();
	test_add();
	test_add_tail();
	test_del();
	test_empty();
	test_position();
	test_layout();

	printf("\n==================================\n");
	printf("All tests passed! ✓\n");

	return 0;
}
