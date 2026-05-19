h33679
s 00000/00000/00000
d R 1.2 01/10/20 13:34:51 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBErr.H
c Name history : 1 0 src/db/BTDBErr.H
e
s 00029/00000/00000
d D 1.1 01/10/20 13:34:50 bmc 1 0
c date and time created 01/10/20 13:34:50 by bmc
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
/*    FILE: BTDBErr.H                                           */
/*    DATE: Wed Jan 26 23:50:16 1994                            */
/****************************************************************/

#ifndef BTDBERR_H
#define BTDBERR_H

#include "BTConfig.H"

#define ERRBTDBNOERR		0	// No error occurred
#define ERRBTDB			-1	// Unknown database error
#define ERRBTDBCREATE		-2	// Failed to create file
#define ERRBTDBOPEN		-3	// Failed to open file
#define ERRBTDBREMOVE		-4	// Failed to remove file
#define ERRBTDBTIMEOUT		-5	// Timed out waiting to lock file
#define ERRBTDBCLOSE		-6	// Failed to close file
#define ERRBTDBREAD		-7	// Failed to read from file
#define ERRBTDBWRITE		-8	// Failed to write to file
#define ERRBTDBSTAT		-9	// Failed to stat file
#define ERRBTDBLOCK		-10	// Failed to lock file
#define ERRBTDBSEEK		-11	// Failed to seek file
#define ERRBTDBCORRUPT		-12	// Database file appears to be corrupt

extern char *BTDBErrMsg(short errcode);

#endif
E 1
