h32022
s 00000/00000/00000
d R 1.2 01/10/20 13:34:47 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/BTDBLock.H
c Name history : 1 0 src/db/BTDBLock.H
e
s 00048/00000/00000
d D 1.1 01/10/20 13:34:46 bmc 1 0
c date and time created 01/10/20 13:34:46 by bmc
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
/*    FILE: BTDBLock.H                                          */
/*    DATE: Sat Sep  3 18:49:34 1994                            */
/****************************************************************/

#ifndef BTDBLOCK_H
#define BTDBLOCK_H

#include "BTConfig.H"

#include <fcntl.h>

#include "BTDBErr.H"

class BTDBLock {

private:
  BTDBLock(const BTDBLock& other);
  BTDBLock& operator=(const BTDBLock& other);

protected:
  short lockRegion(int filedes, int cmd, int type, off_t offset, off_t length);
  pid_t testRegion(int filedes, int type, off_t offset, off_t length);
  

protected:
  int filedes_;
  int locked_;
  off_t offset_;
  off_t length_;

public:
  BTDBLock(int filedes, off_t offset = 0, off_t length = 0);
  ~BTDBLock();

  virtual short lock() = 0;
  virtual short lockw() = 0;
  virtual short unlock();

  virtual pid_t testLock() = 0;
  
  int locked() { return locked_; }

};

#endif
E 1
