h34364
s 00000/00000/00000
d R 1.2 01/10/20 13:34:49 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTNetwork.H
c Name history : 1 0 src/db/BTNetwork.H
e
s 00035/00000/00000
d D 1.1 01/10/20 13:34:48 bmc 1 0
c date and time created 01/10/20 13:34:48 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTNetwork.H                                         */
/*    DATE: Fri Dec 23 02:27:16 1994                            */
/****************************************************************/

#ifndef BTNETWORK_H
#define BTNETWORK_H

#include "BTConfig.H"

#include <netinet/in.h>

#define BTNET_PUTLONG(bufptr, tmp, x) \
tmp = htonl(x); \
bcopy((char *) &tmp, (char *) bufptr, sizeof(unsigned long)); \
bufptr += sizeof(unsigned long)

#define BTNET_PUTSHORT(bufptr, tmp, x) \
tmp = htons(x); \
bcopy((char *) &tmp, (char *) bufptr, sizeof(unsigned short)); \
bufptr += sizeof(unsigned short)

#define BTNET_GETLONG(bufptr, tmp, x) \
bcopy((char *) bufptr, (char *) &tmp, sizeof(unsigned long)); \
bufptr += sizeof(unsigned long); \
x = ntohl(tmp)

#define BTNET_GETSHORT(bufptr, tmp, x) \
bcopy((char *) bufptr, (char *) &tmp, sizeof(unsigned short)); \
bufptr += sizeof(unsigned short); \
x = ntohs(tmp)

#endif
E 1
