#ifndef _MATH_H
#define _MATH_H
double fabs(double);
double floor(double);
double ceil(double);
double sqrt(double);
double pow(double, double);
float fabsf(float);
double NAN;
double INFINITY;
#define isnan(x) __builtin_isnan(x)
#define isinf(x) __builtin_isinf(x)
#endif
