h02561
s 00000/00000/00000
d R 1.2 01/10/20 13:35:38 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/Mem.H
c Name history : 1 0 src/game/Mem.H
e
s 00030/00000/00000
d D 1.1 01/10/20 13:35:37 bmc 1 0
c date and time created 01/10/20 13:35:37 by bmc
e
u
U
f e 0
t
T
I 1
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
E 1
