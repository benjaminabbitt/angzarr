/* SPDX-License-Identifier: GPL-2.0 */
/*
 * Standalone C reference implementation for doubly-linked lists
 *
 * Based on Linux kernel include/linux/list.h
 * Simplified for userspace compilation and testing
 *
 * Copyright (C) Linux Kernel Authors
 * Adapted for Angzarr test reference
 */

#ifndef _C_REFERENCE_LIST_H
#define _C_REFERENCE_LIST_H

#include <stddef.h>
#include <stdbool.h>

/*
 * Simple doubly linked list implementation.
 *
 * Binary-compatible with Linux kernel struct list_head
 */
struct list_head {
	struct list_head *next, *prev;
};

/*
 * Static initializer for a list head
 */
#define LIST_HEAD_INIT(name) { &(name), &(name) }

/*
 * Declare and initialize a list head
 */
#define LIST_HEAD(name) \
	struct list_head name = LIST_HEAD_INIT(name)

/*
 * Initialize a list head to point to itself
 */
static inline void INIT_LIST_HEAD(struct list_head *list)
{
	list->next = list;
	list->prev = list;
}

/*
 * Internal function: Insert entry between prev and next
 */
static inline void __list_add(struct list_head *new,
			       struct list_head *prev,
			       struct list_head *next)
{
	next->prev = new;
	new->next = next;
	new->prev = prev;
	prev->next = new;
}

/*
 * list_add - add a new entry
 * @new: new entry to be added
 * @head: list head to add it after
 *
 * Insert a new entry after the specified head.
 * This is good for implementing stacks.
 */
static inline void list_add(struct list_head *new, struct list_head *head)
{
	__list_add(new, head, head->next);
}

/*
 * list_add_tail - add a new entry
 * @new: new entry to be added
 * @head: list head to add it before
 *
 * Insert a new entry before the specified head.
 * This is useful for implementing queues.
 */
static inline void list_add_tail(struct list_head *new, struct list_head *head)
{
	__list_add(new, head->prev, head);
}

/*
 * Internal function: Delete entry by making prev/next point to each other
 */
static inline void __list_del(struct list_head *prev, struct list_head *next)
{
	next->prev = prev;
	prev->next = next;
}

/*
 * list_del - deletes entry from list
 * @entry: the element to delete from the list
 *
 * Note: list_empty() on entry does not return true after this, the entry is
 * in an undefined state.
 */
static inline void list_del(struct list_head *entry)
{
	__list_del(entry->prev, entry->next);
	entry->next = NULL;
	entry->prev = NULL;
}

/*
 * list_empty - tests whether a list is empty
 * @head: the list to test
 */
static inline bool list_empty(const struct list_head *head)
{
	return head->next == head;
}

/*
 * list_is_head - tests whether @list is the list @head
 * @list: the entry to test
 * @head: the head of the list
 */
static inline bool list_is_head(const struct list_head *list,
				 const struct list_head *head)
{
	return list == head;
}

/*
 * list_is_first -- tests whether @list is the first entry in list @head
 * @list: the entry to test
 * @head: the head of the list
 */
static inline bool list_is_first(const struct list_head *list,
				  const struct list_head *head)
{
	return list->prev == head;
}

/*
 * list_is_last - tests whether @list is the last entry in list @head
 * @list: the entry to test
 * @head: the head of the list
 */
static inline bool list_is_last(const struct list_head *list,
				 const struct list_head *head)
{
	return list->next == head;
}

#endif /* _C_REFERENCE_LIST_H */
