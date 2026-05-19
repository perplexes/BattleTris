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
