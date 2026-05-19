h60264
s 00000/00000/00000
d R 1.2 01/10/20 13:35:27 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTDebug.H
c Name history : 1 0 src/game/BTDebug.H
e
s 00026/00000/00000
d D 1.1 01/10/20 13:35:26 bmc 1 0
c date and time created 01/10/20 13:35:26 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTDebug.H                                           */
/*    DATE: Wed Apr 13 16:48:20 1994                            */
/****************************************************************/

#ifndef BTDEBUG_H
#define BTDEBUG_H

#include "BTConfig.H"
#include "BTConstants.H"

#ifndef NDEBUG

#include <iostream.h>

#define BTDebug(x)  cerr << BT_VERSION << " [" << __FILE__ << ", " << __LINE__ << "] DEBUG: " << x << endl

#else

#define BTDebug(x) 

#endif

#endif
E 1
