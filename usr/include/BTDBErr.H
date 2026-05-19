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
