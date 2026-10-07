#include <math.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#define N 4000000
static uint64_t st;
static double rf(void){ st^=st<<13; st^=st>>7; st^=st<<17; return (double)(st>>11)/(double)(1ULL<<53); }
static volatile float vf; static volatile double vd;
static const char *dir;
static FILE *open_(const char *n){ char p[512]; snprintf(p,512,"%s/%s",dir,n); return fopen(p,"wb"); }
#define RUN32(name,lo,hi,expr) { st=0x9E3779B97F4A7C15ULL; FILE*f=open_(name); for(int i=0;i<N;i++){ float a=(float)(lo+(hi-lo)*rf()); float b=(float)(lo+(hi-lo)*rf()); vf=a; a=vf; float x=(expr); uint64_t u=0; memcpy(&u,&x,4); fwrite(&u,8,1,f);} fclose(f);}
#define RUN64(name,lo,hi,expr) { st=0x9E3779B97F4A7C15ULL; FILE*f=open_(name); for(int i=0;i<N;i++){ double a=lo+(hi-lo)*rf(); double b=lo+(hi-lo)*rf(); vd=a; a=vd; double x=(expr); uint64_t u; memcpy(&u,&x,8); fwrite(&u,8,1,f);} fclose(f);}
int main(int argc,char**argv){ dir=argv[1]; char cmd[600]; snprintf(cmd,600,"mkdir -p %s",dir); system(cmd);
 RUN32("f32.sin",-20.0,20.0,sinf(a)) RUN32("f32.cos",-20.0,20.0,cosf(a)) RUN32("f32.sin_pi",-3.1415926,3.1415926,sinf(a)) RUN32("f32.cos_pi",-3.1415926,3.1415926,cosf(a))
 RUN32("f32.atan2",-500.0,500.0,atan2f(a,b)) RUN32("f32.exp",-30.0,5.0,expf(a)) RUN32("f32.powf",0.0,4.0,powf(a,b))
 RUN32("f32.ln",0.0,100.0,logf(a)) RUN32("f32.acos",-1.0,1.0,acosf(a)) RUN32("f32.hypot",-500.0,500.0,hypotf(a,b)) RUN32("f32.sqrt",0.0,1e6,sqrtf(a))
 RUN64("f64.sin",-20.0,20.0,sin(a)) RUN64("f64.cos",-20.0,20.0,cos(a)) RUN64("f64.atan2",-1e4,1e4,atan2(a,b)) RUN64("f64.hypot",-1e4,1e4,hypot(a,b))
 RUN64("f64.powf",0.0,4.0,pow(a,b)) RUN64("f64.exp",-30.0,5.0,exp(a)) RUN64("f64.ln",0.0,100.0,log(a)) RUN64("f64.sqrt",0.0,1e6,sqrt(a))
 return 0; }
