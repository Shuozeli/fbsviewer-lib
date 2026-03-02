#ifndef _STDIO_H
#define _STDIO_H
typedef __SIZE_TYPE__ size_t;
typedef struct _FILE FILE;
#define NULL ((void*)0)
#define EOF (-1)
extern FILE *stderr;
int fprintf(FILE *, const char *, ...);
int printf(const char *, ...);
int snprintf(char *, size_t, const char *, ...);
int vsnprintf(char *, size_t, const char *, __builtin_va_list);
int fputc(int, FILE *);
int fputs(const char *, FILE *);
int fclose(FILE *);
FILE *fdopen(int, const char *);
#endif
