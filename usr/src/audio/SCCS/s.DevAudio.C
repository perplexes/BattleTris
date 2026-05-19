h00206
s 00117/00110/00208
d D 1.3 01/10/21 19:25:01 bmc 4 3
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00080/00024/00238
d D 1.2 01/10/21 01:52:44 bmc 3 1
c 1000007 audio relies on broken, unshipped header files, libraries
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:06 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/audio/DevAudio.C
c Name history : 1 0 src/audio/DevAudio.C
e
s 00262/00000/00000
d D 1.1 01/10/20 13:35:05 bmc 1 0
c date and time created 01/10/20 13:35:05 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: DevAudio.C                                          */
/*    DATE: Thu Feb 10 14:52:44 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if HAVE_SUNAUDIO
D 3
# include <multimedia/audio_device.h>
# include <multimedia/libaudio.h>
E 3
I 3
/*
 * It's a goddamned crime that this isn't defined anywhere...
 */
typedef struct {
	uint32_t	audio_magic;
	uint32_t	audio_hdr_size;
	uint32_t	audio_data_size;
	uint32_t	audio_encoding;
	uint32_t	audio_sample_rate;
	uint32_t	audio_channels;
} audio_hdr_t;

/*
 * Can you fucking believe this shit?  Do we hate our customers?
 */
#define AUDIO_FILE_MAGIC		((uint32_t)0x2e736e64)
#define AUDIO_FILE_ENCODING_MULAW_8	(1)	/* 8-bit ISDN u-law */

#include <sys/audioio.h>
E 3
#endif

#include <sys/stat.h>

#include <iostream.h>
#include <stdio.h>
#include <fcntl.h>
#include <errno.h>

#if STDC_HEADERS
# include <stdlib.h>
#endif

#if HAVE_UNISTD_H
# include <unistd.h>
#endif

#include "SigReceiver.H"
#include "DevAudio.H"

const char *DEVAUDIO_PATH = "/dev/audio";
const unsigned int DEVAUDIO_TIMEOUT = 15;

static int audio_fd = -1;

class DevAudioHandler : public SigHandler {
protected:
  int master_fd_, audio_fd_;
public:
  DevAudioHandler(int master_fd, int audio_fd,
                  SigDisposition disp = SIG_ENABLED)
  : SigHandler(disp), master_fd_(master_fd), audio_fd_(audio_fd) {}
  virtual ~DevAudioHandler() {}
  void handle();
};

void DevAudioHandler::handle()
{
  close(master_fd_);
  close(audio_fd_);
  exit(0);
}

class DevAudioTimer : public SigHandler {
public:
  DevAudioTimer(SigDisposition disp = SIG_ENABLED) : SigHandler(disp) {}
  virtual ~DevAudioTimer() {}
  void handle();
};

void DevAudioTimer::handle()
{
  // Don't do anything here -- just want to cause blocking read to fail
  // in DevAudio::SlaveLoop indicating that timer has expired
}

D 4
DevAudio::DevAudio(int headphones, char *root)
E 4
I 4
DevAudio::DevAudio(int headphones, const char *root)
E 4
: master_fd_(-1), fd_(-1), slave_fd_(-1), valid_(0)
{  
D 4
  struct stat st;
  int pfd[2];	
E 4
I 4
	struct stat st;
	int pfd[2];	
E 4

D 4
  if(root == 0) 
    root_[0] = 0;
  else
    strcpy(root_, root);
E 4
I 4
	if (root == 0) 
		root_[0] = 0;
	else
		strcpy(root_, root);
E 4

#if HAVE_SUNAUDIO 
D 4
  if(stat(DEVAUDIO_PATH, &st) < 0) {
    cerr << "BattleTris: Failed to stat audio device" << endl;
    return;
  }
E 4
I 4
	if (stat(DEVAUDIO_PATH, &st) < 0) {
		cerr << "BattleTris: Failed to stat audio device" << endl;
		return;
	}
E 4
 
D 4
  if((fd_ = open(DEVAUDIO_PATH, O_WRONLY)) == -1) {
    cerr << "BattleTris: Failed to open audio device" << endl;
    return;
  }
E 4
I 4
	if ((fd_ = open(DEVAUDIO_PATH, O_WRONLY)) == -1) {
		cerr << "BattleTris: Failed to open audio device" << endl;
		return;
	}
E 4

D 4
  close(fd_);
E 4
I 4
	close(fd_);
E 4

D 4
  // Need to open a pipe, and fork() a slave
E 4
I 4
	/*
	 * Need to open a pipe, and fork() a slave
	 */
	if (pipe(pfd) < 0) {
		cerr << "BattleTris: Failed to open audio pipe" << endl;
		return;
	}   
E 4

D 4
  if(pipe(pfd) < 0) {
    cerr << "BattleTris: Failed to open audio pipe" << endl;
    return;
  }   
E 4
I 4
	if ((slave_pid_ = fork()) < 0) {
		cerr << "BattleTris: Failed to fork audio slave" << endl;
		return;
	}
E 4

D 4
  if((slave_pid_ = fork()) < 0) {
    cerr << "BattleTris: Failed to fork audio slave" << endl;
    return;
  }
E 4
I 4
	/*
	 * If fork succeeded, then we're alive enough to indicate that
	 * BTStartup should not delete us
	 */
  	valid_ = 1;
E 4

D 4
  // If fork succeeded, then we\'re alive enough to indicate that
  // BTStartup should not delete us
E 4
I 4
	if (slave_pid_ == 0) {
		if ((fd_ = open(DEVAUDIO_PATH, O_WRONLY | O_NDELAY)) == -1)
			fd_ = open ("/dev/null", O_WRONLY);
E 4

D 4
  valid_ = 1;
E 4
I 4
		if (headphones) {
			audio_info info;
E 4

D 4
  if(slave_pid_ == 0) { // We are in the child
    if((fd_ = open(DEVAUDIO_PATH, O_WRONLY | O_NDELAY)) == -1)
      fd_ = open ("/dev/null", O_WRONLY);
E 4
I 4
			cout << "BattleTris: Headphone sound enabled" << endl;
			AUDIO_INITINFO(&info);
			info.play.port = 0;
			info.play.port |= AUDIO_HEADPHONE;
E 4

D 4
    if(headphones) {
      cout << "BattleTris: Headphone sound enabled" << endl;
E 4
I 4
			info.play.encoding = AUDIO_ENCODING_ULAW;
			info.play.precision = 8;
			info.play.channels = 1;
			info.play.sample_rate = 8000;
E 4

D 4
      audio_info info;
      AUDIO_INITINFO(&info);
      info.play.port = 0;
      info.play.port |= AUDIO_HEADPHONE;
E 4
I 4
			ioctl(audio_fd, AUDIO_SETINFO, &info);
		} else {
			audio_info info;
E 4

D 4
      // Configure the AMD device

      info.play.encoding = AUDIO_ENCODING_ULAW;
      info.play.precision = 8;
      info.play.channels = 1;
      info.play.sample_rate = 8000;

      ioctl(audio_fd, AUDIO_SETINFO, &info);
    } else {
      audio_info info;
      AUDIO_INITINFO(&info);
      info.play.port = 0;
      info.play.port |= AUDIO_SPEAKER;
      ioctl(audio_fd, AUDIO_SETINFO, &info);
    }
E 4
I 4
			AUDIO_INITINFO(&info);
			info.play.port = 0;
			info.play.port |= AUDIO_SPEAKER;
			ioctl(audio_fd, AUDIO_SETINFO, &info);
		}
E 4
      
D 4
    close(pfd[1]);      // we don\'t want to be writing
    master_fd_ = pfd[0];
E 4
I 4
		close(pfd[1]);
		master_fd_ = pfd[0];
E 4

D 4
    SlaveLoop();
  }
E 4
I 4
		SlaveLoop();
	}
E 4

D 4
  close(pfd[0]);
  slave_fd_ = pfd[1];
E 4
I 4
	close(pfd[0]);
	slave_fd_ = pfd[1];
E 4
#else
D 4
  valid_ = 1;
E 4
I 4
	valid_ = 1;
E 4
#endif
}

D 4
void DevAudio::QueueFile(char *filename)
E 4
I 4
void
DevAudio::QueueFile(const char *filename)
E 4
{
#if HAVE_SUNAUDIO
D 4
  char fullpath[1024];
E 4
I 4
	char fullpath[1024];
E 4

D 4
  strcpy(fullpath, root_);
  strcat(fullpath, filename);
E 4
I 4
	strcpy(fullpath, root_);
	strcat(fullpath, filename);
E 4

D 4
  // All we need to do is send this down the pipe 
  // (just return if there isn't one)
E 4
I 4
	/*
	 * All we need to do is send this down the pipe (just return if
	 * there isn't one).
	 */
	if(slave_fd_ == -1)
		return;
E 4

D 4
  if(slave_fd_ == -1)
    return;

  // Send the path plus the null

  write(slave_fd_, fullpath, strlen(fullpath) + 1);
E 4
I 4
	/*
	 * Send the path plus the null
	 */
	write(slave_fd_, fullpath, strlen(fullpath) + 1);
E 4
#endif
}

D 4
void DevAudio::SlaveLoop()
E 4
I 4
void
DevAudio::SlaveLoop()
E 4
{
#if HAVE_SUNAUDIO
D 4
  DevAudioHandler handler(master_fd_, fd_);
  DevAudioTimer timer;
E 4
I 4
	DevAudioHandler handler(master_fd_, fd_);
	DevAudioTimer timer;
E 4

D 4
  SigReceiver sigrec;
  pid_t ppid = getppid();
E 4
I 4
	SigReceiver sigrec;
	pid_t ppid = getppid();
E 4

D 4
  char filename[1024];
  register int i;
E 4
I 4
	char filename[1024];
	int i;
E 4

D 4
  sigrec.reset();
E 4
I 4
	sigrec.reset();
E 4

D 4
  sigrec.install(SIGINT, &handler);
  sigrec.install(SIGHUP, &handler);
  sigrec.install(SIGTERM, &handler);
  sigrec.install(SIGPIPE, &handler);
  sigrec.install(SIGALRM, &timer);
E 4
I 4
	sigrec.install(SIGINT, &handler);
	sigrec.install(SIGHUP, &handler);
	sigrec.install(SIGTERM, &handler);
	sigrec.install(SIGPIPE, &handler);
	sigrec.install(SIGALRM, &timer);
E 4

D 4
  for(;;) {
    i = 0;
E 4
I 4
	for (;;) {
		i = 0;
E 4

D 4
    do {
      alarm(DEVAUDIO_TIMEOUT); // Set the alarm timer
E 4
I 4
		do {
			alarm(DEVAUDIO_TIMEOUT);
E 4

D 4
      while(read(master_fd_, &filename[i], 1) != 1) {
        if(kill(ppid, 0) < 0 && errno == ESRCH) {
          cerr << "BattleTris: Sound daemon aborting because game has died\n";
          close(master_fd_);
          close(fd_);
          exit(0);
        } else {
          alarm(0); // Clear and reset the alarm timer
          alarm(DEVAUDIO_TIMEOUT);
        }
      }
E 4
I 4
			while (read(master_fd_, &filename[i], 1) != 1) {
				if (kill(ppid, 0) < 0 && errno == ESRCH) {
					cerr << "BattleTris: Sound daemon "
					    "aborting because game has died\n";
					close(master_fd_);
					close(fd_);
					exit(0);
				} else {
					alarm(0); // Reset the alarm timer
					alarm(DEVAUDIO_TIMEOUT);
        			}
			}
E 4

D 4
      alarm(0); // Clear the alarm timer
E 4
I 4
			alarm(0); // Clear the alarm timer
E 4

D 4
    } while(filename[i++] != 0);
E 4
I 4
		} while(filename[i++] != 0);
E 4

D 4
    DumpFile(&filename[0]);
  }
E 4
I 4
		DumpFile(&filename[0]);
	}
E 4
#endif
}

D 3
void DevAudio::DumpFile(char *filename)
E 3
I 3
static uint32_t
au2native(uint32_t val)
E 3
{
I 3
#ifdef sparc
	return (val);
#else
#ifdef i386
	uint32_t ret;

	((char *)&ret)[0] = ((char *)&val)[3];
	((char *)&ret)[1] = ((char *)&val)[2];
	((char *)&ret)[2] = ((char *)&val)[1];
	((char *)&ret)[3] = ((char *)&val)[0];

	return (ret);
#else
#error Unrecognized platform.
#endif
#endif
}

D 4
void DevAudio::DumpFile(char *file)
E 4
I 4
void
DevAudio::DumpFile(const char *file)
E 4
{
E 3
#if HAVE_SUNAUDIO
D 3
  int file_fd;
  Audio_hdr file_hdr;
E 3
I 3
	int fd;
	audio_hdr_t hdr;
	uint32_t *i;
	ssize_t cnt, err;
	char buf[64 * 1024];
E 3

D 3
  if((file_fd = open(filename, O_RDONLY, 0)) < 0)
    return;
E 3
I 3
	if ((fd = open(file, O_RDONLY)) == -1) {
		cerr << "BattleTris: Failed to open sound file '" <<
		    file << "'" << endl;
		return;
	}
E 3

D 3
  int err = audio_read_filehdr (file_fd, &file_hdr, (char *) 0, 0);
E 3
I 3
	if (read(fd, &hdr, sizeof (hdr)) != sizeof (hdr)) {
		cerr << "BattleTris: couldn't read header for sound file '" <<
		    file << "'" << endl;
		return;
	}
E 3

D 3
  if(err != AUDIO_SUCCESS)
    return;
  
  static char buf[64 * 1024];
  int cnt = 0;
E 3
I 3
	for (i = (uint32_t *)&hdr;
	    (uintptr_t)i - (uintptr_t)&hdr < sizeof (hdr); i++)
		*i = au2native(*i);
E 3

D 3
  // Wait until audio device has drained...
  ioctl (fd_, AUDIO_DRAIN, 0);
E 3
I 3
	if (hdr.audio_magic != AUDIO_FILE_MAGIC) {
		cerr << file << " is not an audio file (bad magic)" << endl;
		return;
	}
E 3

D 3
  while((cnt = read (file_fd, (char *) buf, sizeof (buf))) >= 0) {
    while (((err = write (fd_, (char *) buf, cnt)) == -1) && errno == EAGAIN)
      ;
E 3
I 3
	if (hdr.audio_encoding != AUDIO_FILE_ENCODING_MULAW_8) {
		cerr << file << " has unsupported audio encoding " <<
		    hdr.audio_encoding << endl;
		return;
	}
E 3

D 3
    if(err != cnt)
      break;
    if(cnt == 0)
      break;
  }
E 3
I 3
	lseek(fd, hdr.audio_hdr_size, SEEK_SET);
E 3

D 3
  close(file_fd);
E 3
I 3
	ioctl(fd_, AUDIO_DRAIN, 0);

	while ((cnt = read(fd, buf, sizeof (buf))) >= 0) {
		while (((err = write(fd_, buf, cnt)) == -1) && errno == EAGAIN)
			continue;
		
		if (err != cnt || cnt == 0)
			break;
	}

	close(fd);
E 3
#endif
}    

DevAudio::~DevAudio()
{
#if HAVE_SUNAUDIO
D 4
  if(valid_) {
    kill(slave_pid_, SIGTERM);
    close(master_fd_);
    close(slave_fd_);
    close(fd_);
  }
E 4
I 4
	if (valid_) {
		kill(slave_pid_, SIGTERM);
		close(master_fd_);
		close(slave_fd_);
		close(fd_);
	}
E 4
#endif
}
E 1
