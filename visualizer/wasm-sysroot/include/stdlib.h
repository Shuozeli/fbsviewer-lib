#ifndef _STDLIB_H
#define _STDLIB_H
typedef __SIZE_TYPE__ size_t;
#define NULL ((void*)0)
void *malloc(size_t);
void *calloc(size_t, size_t);
void *realloc(void *, size_t);
void free(void *);
void abort(void) __attribute__((noreturn));
int abs(int);
long labs(long);
#define EXIT_FAILURE 1
#define EXIT_SUCCESS 0
void exit(int) __attribute__((noreturn));
#endif
