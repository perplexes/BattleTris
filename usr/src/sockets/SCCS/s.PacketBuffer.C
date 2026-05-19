h20478
s 00000/00000/00000
d R 1.2 01/10/20 13:35:02 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/PacketBuffer.C
c Name history : 1 0 src/sockets/PacketBuffer.C
e
s 00074/00000/00000
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
/*    FILE: PacketBuffer.C                                      */
/*    DATE: Fri Dec 23 00:01:10 1994                            */
/****************************************************************/

#include "BTConfig.H"

#include <netinet/in.h>

#include "PacketBuffer.H"

short PacketBuffer::sendpacket(netlong type, netlong nbytes, char *data)
{
  netlong header[2];
  short err;

  header[0] = htonl(type);
  header[1] = htonl(nbytes);

  if((err = sock_->sendbuf((char *) header, sizeof(header))) < 0)
    return err;

  if(!nbytes)
    return err;

  return sock_->sendbuf(data, nbytes);
}

short PacketBuffer::recvpacket()
{
  short err;

  if((err = sock_->recvbuf((char *) header_, sizeof(header_))) < 0)
    return err;

  header_[0] = ntohl(header_[0]);
  header_[1] = ntohl(header_[1]);

  if(!header_[1])
    return err;

  if(header_[1] > buflen_) {
    if(dynabuf_)
      delete [] dynabuf_;
    buflen_ = ((header_[1] >> sizeof(netlong)) + 1) << sizeof(netlong);
    dynabuf_ = new char [buflen_];
    bufptr_ = dynabuf_;
  }

  return sock_->recvbuf(bufptr_, header_[1]);
}

short PacketBuffer::peekpacket()
{
  short err;

  if((err = sock_->peekbuf((char *) header_, sizeof(header_))) < 0)
    return err;

  header_[0] = ntohl(header_[0]);
  header_[1] = ntohl(header_[1]);

  if(header_[1] > buflen_) {
    if(dynabuf_)
      delete [] dynabuf_;
    buflen_ = ((header_[1] >> sizeof(netlong)) + 1) << sizeof(netlong);
    dynabuf_ = new char [buflen_];
    bufptr_ = dynabuf_;
  }

  return sock_->peekbuf(bufptr_, header_[1]);
}
E 1
