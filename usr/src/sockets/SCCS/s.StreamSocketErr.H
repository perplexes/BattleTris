h55227
s 00000/00000/00000
d R 1.2 01/10/20 13:35:02 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/StreamSocketErr.H
c Name history : 1 0 src/sockets/StreamSocketErr.H
e
s 00031/00000/00000
d D 1.1 01/10/20 13:35:01 bmc 1 0
c date and time created 01/10/20 13:35:01 by bmc
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
E 1
