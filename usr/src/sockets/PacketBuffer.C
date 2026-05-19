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
