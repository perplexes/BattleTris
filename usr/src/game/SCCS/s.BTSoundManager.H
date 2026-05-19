h40333
s 00001/00003/00058
d D 1.3 01/10/22 22:16:07 dep 4 3
c 1000015 sounds don't play under libumem
e
s 00024/00025/00037
d D 1.2 01/10/21 19:25:12 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:34 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/BTSoundManager.H
c Name history : 1 0 src/game/BTSoundManager.H
e
s 00062/00000/00000
d D 1.1 01/10/20 13:35:33 bmc 1 0
c CodeManager Uniquification: src/game/BTSoundManager.H
c date and time created 01/10/20 13:35:33 by bmc
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
/*    FILE: BTSound.H                                           */
/*    ASSN:                                                     */
/*    DATE: Sun Feb 20 15:17:38 1994                            */
/****************************************************************/

#ifndef BTSOUNDMANAGER_H
#define BTSOUNDMANAGER_H

#include "BTConfig.H"

#include "DevAudio.H"
#include "Block.H"

#include "BTRingNode.H"
#include "BTConstants.H"
#include "BTLine.H"
#include <string.h>

#define BT_MAX_SND_FILENAME 256

struct BTSoundFile {
D 3
  char name_[BT_MAX_SND_FILENAME];
  int used_;
  BTSoundFile( char *name = "", int used = 0 ) : used_(used) {
    strncpy( name_, name, strlen(name) > BT_MAX_SND_FILENAME ? 
	     BT_MAX_SND_FILENAME : strlen(name) );
    name_[BT_MAX_SND_FILENAME-1] = 0;
  }
E 3
I 3
	char name_[BT_MAX_SND_FILENAME];
	int used_;
	BTSoundFile(const char *name = "", int used = 0) : used_(used)
	{
D 4
		strncpy(name_, name, strlen(name) > BT_MAX_SND_FILENAME ? 
		    BT_MAX_SND_FILENAME : strlen(name) );
		name_[BT_MAX_SND_FILENAME-1] = 0;
E 4
I 4
		(void) strlcpy(name_, name, BT_MAX_SND_FILENAME);
E 4
	}
E 3
};

class BTSoundManager : public BTRingNode {
private:
D 3
  DevAudio *dev_;
E 3
I 3
	DevAudio *dev_;
	int paused_;
E 3

D 3
  int paused_;
E 3
I 3
	// The blocks of filenames to select from
	Block<BTSoundFile *> welcome_, start_, bad_move_, near_death_,
	    tetris_, launched_, survived_, won_, lost_, launch_wimpy_;
	int happy_;
E 3

D 3
  // The blocks of filenames to select from
  Block<BTSoundFile *> welcome_, start_, bad_move_, near_death_, tetris_,
    launched_, survived_, won_, lost_, launch_wimpy_;
E 3
I 3
	void play_random (Block<BTSoundFile *> *block);
	void read_dir (const char *dir_name, Block<BTSoundFile *> *block);
	void cleanBlock( Block<BTSoundFile *> *block );
E 3

D 3
  int happy_;

  void play_random (Block<BTSoundFile *> *block);
  void read_dir (char *dir_name, Block<BTSoundFile *> *block);
  void cleanBlock( Block<BTSoundFile *> *block );

E 3
public:
D 3
  BTSoundManager(DevAudio *dev);
  ~BTSoundManager();  
E 3
I 3
	BTSoundManager(DevAudio *dev);
	~BTSoundManager();  
E 3

D 3
  void welcome();
  void playJeopardy();
  void start();
E 3
I 3
	void welcome();
	void playJeopardy();
	void start();
E 3

D 3
  void setdev(DevAudio *dev) { dev_ = dev; }
  void receive(BTRingPacket *packet);  
E 3
I 3
	void setdev(DevAudio *dev) { dev_ = dev; }
	void receive(BTRingPacket *packet);  
E 3
};

#endif
E 1
