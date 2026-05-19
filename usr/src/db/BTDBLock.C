/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: BTDBLock.C                                          */
/*    DATE: Sat Sep  3 20:53:59 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#if STDC_HEADERS
# include <stdlib.h>
#endif

#include <assert.h>
#include <errno.h>

#include "BTDBLock.H"

BTDBLock::BTDBLock(int filedes, off_t offset, off_t length)
: filedes_(filedes), offset_(offset), length_(length), locked_(0)
{
  assert(filedes >= 0);
  assert(offset >= 0);
  assert(length >= 0);
}

BTDBLock::~BTDBLock()
{
  if(locked_)
    unlock();
}

short BTDBLock::lockRegion(int filedes, int cmd, int type, off_t offset,
			  off_t length)
{
  struct flock lock;

  lock.l_type = type;
  lock.l_start = offset;
  lock.l_whence = SEEK_SET;
  lock.l_len = length;

  if(fcntl(filedes, cmd, &lock) < 0) {
    if(errno == EDEADLK) {

      struct timeval t;
      t.tv_sec = 0;
      t.tv_usec = rand() % 1000000;

      select(0, (SELECTARGTYPE *) 0, (SELECTARGTYPE *) 0,
             (SELECTARGTYPE *) 0, &t);

      if(fcntl(filedes, cmd, &lock) < 0)
	return ERRBTDBLOCK;
      
    } else {
      return ERRBTDBLOCK;
    }
  }

  return ERRBTDBNOERR;
}

pid_t BTDBLock::testRegion(int filedes, int type, off_t offset, off_t length)
{
  struct flock lock;

  lock.l_type = type;
  lock.l_start = offset;
  lock.l_whence = SEEK_SET;
  lock.l_len = length;

  if(fcntl(filedes, F_GETLK, &lock) < 0)
    return 0;

  if(lock.l_type == F_UNLCK)
    return 0;		// region is not currently locked
  else 
    return lock.l_pid;	// return pid of lock owner
}

short BTDBLock::unlock()
{
  short err = ERRBTDBNOERR;

  if(locked_) {
    err = lockRegion(filedes_, F_SETLK, F_UNLCK, offset_, length_);

    if(err == ERRBTDBNOERR)
      locked_ = 0;
  }

  return err;
}
