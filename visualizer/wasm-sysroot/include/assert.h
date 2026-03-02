#ifndef _ASSERT_H
#define _ASSERT_H
void abort(void) __attribute__((noreturn));
#define assert(e) ((void)((e) ? ((void)0) : abort()))
#endif
