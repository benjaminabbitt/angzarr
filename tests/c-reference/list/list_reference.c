/* SPDX-License-Identifier: GPL-2.0 */
/*
 * C reference implementation for list operations
 *
 * This file provides C functions that Rust tests can call to verify
 * that Rust implementations match C behavior exactly.
 *
 * These functions are exported and can be called from Rust via FFI.
 */

#include "list.h"
#include <stddef.h>

/*
 * Export list_head structure size and alignment
 * Rust tests can use these to verify binary compatibility
 */
const size_t C_LIST_HEAD_SIZE = sizeof(struct list_head);
const size_t C_LIST_HEAD_ALIGN = __alignof__(struct list_head);
const size_t C_LIST_HEAD_NEXT_OFFSET = offsetof(struct list_head, next);
const size_t C_LIST_HEAD_PREV_OFFSET = offsetof(struct list_head, prev);

/*
 * Reference implementation: Initialize list head
 *
 * Rust can call this to get expected behavior
 */
void c_ref_list_init(struct list_head *list)
{
	INIT_LIST_HEAD(list);
}

/*
 * Reference implementation: Add entry after head
 */
void c_ref_list_add(struct list_head *new, struct list_head *head)
{
	list_add(new, head);
}

/*
 * Reference implementation: Add entry before head (at tail)
 */
void c_ref_list_add_tail(struct list_head *new, struct list_head *head)
{
	list_add_tail(new, head);
}

/*
 * Reference implementation: Delete entry
 */
void c_ref_list_del(struct list_head *entry)
{
	list_del(entry);
}

/*
 * Reference implementation: Test if list is empty
 */
int c_ref_list_empty(const struct list_head *head)
{
	return list_empty(head) ? 1 : 0;
}

/*
 * Reference implementation: Test if entry is head
 */
int c_ref_list_is_head(const struct list_head *list, const struct list_head *head)
{
	return list_is_head(list, head) ? 1 : 0;
}

/*
 * Reference implementation: Test if entry is first
 */
int c_ref_list_is_first(const struct list_head *list, const struct list_head *head)
{
	return list_is_first(list, head) ? 1 : 0;
}

/*
 * Reference implementation: Test if entry is last
 */
int c_ref_list_is_last(const struct list_head *list, const struct list_head *head)
{
	return list_is_last(list, head) ? 1 : 0;
}
