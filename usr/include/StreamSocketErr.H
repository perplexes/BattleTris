/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: StreamSocketErr.H                                   */
/*    DATE: Wed Jan 26 23:50:16 1994                            */
/****************************************************************/

#ifndef STREAMSOCKETERR_H
#define STREAMSOCKETERR_H

#include "BTConfig.H"

#define ERRSTREAMNOERR		0	// No error occurred
#define ERRSTREAMSOCKET		-1	// Unknown StreamSocket error
#define ERRSTREAMBIND		-2	// Failed to bind name to socket
#define ERRSTREAMNAME		-3	// Failed to obtain name for socket
#define ERRSTREAMCLOSE		-4	// Failed to close socket
#define ERRSTREAMCONNECT	-5	// Failed to connect socket
#define ERRSTREAMLISTEN		-6	// Failed to enable listening
#define ERRSTREAMSELECT		-7	// Failed to select on socket
#define ERRSTREAMACCEPT		-8	// Failed to accept connection
#define ERRSTREAMSEND		-9	// Failed to send data
#define ERRSTREAMRECV		-10	// Failed to receive data
#define ERRSTREAMFLAG		-11	// Failed to set socket flag
#define ERRSTREAMOPT		-12	// Failed to set socket option
#define ERRSTREAMBROKEN		-13	// Connection unexpectedly broke
#define ERRSTREAMTIMEOUT	-14	// Timed out waiting for data

extern char *StreamSocketErrMsg(short errcode);

#endif
