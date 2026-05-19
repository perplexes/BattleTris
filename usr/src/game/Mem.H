/****************************************************************/
/*    NAME:                                                     */
/*    ACCT: cgh                                                 */
/*    FILE: Mem.H                                               */
/*    ASGN:                                                     */
/*    DATE: Tue Oct 31 11:59:03 1995                            */
/****************************************************************/


#ifndef MEM_HEADER
#define MEM_HEADER

#include <stdlib.h>

#ifdef new
#undef new
#endif
#ifdef delete
#undef delete
#endif

void *operator new( size_t, char filename[] = "", unsigned int = 0);
void operator delete( void * );
//void operator delete[]( void * );

extern void cleanUpMem();

#define new new(__FILE__, __LINE__)

#endif
