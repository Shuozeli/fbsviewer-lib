#ifndef _WCTYPE_H
#define _WCTYPE_H
typedef unsigned int wint_t;
typedef unsigned long wctype_t;
#define WEOF ((wint_t)-1)
int iswdigit(wint_t);
int iswspace(wint_t);
int iswalpha(wint_t);
int iswupper(wint_t);
int iswlower(wint_t);
int towupper(wint_t);
int towlower(wint_t);
#endif
int iswalnum(wint_t);
