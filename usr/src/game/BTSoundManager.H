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
	char name_[BT_MAX_SND_FILENAME];
	int used_;
	BTSoundFile(const char *name = "", int used = 0) : used_(used)
	{
		(void) strncpy(name_, name, BT_MAX_SND_FILENAME - 1);
		name_[BT_MAX_SND_FILENAME - 1] = '\0';
	}
};

class BTSoundManager : public BTRingNode {
private:
	DevAudio *dev_;
	int paused_;

	// The blocks of filenames to select from
	Block<BTSoundFile *> welcome_, start_, bad_move_, near_death_,
	    tetris_, launched_, survived_, won_, lost_, launch_wimpy_;
	int happy_;

	void play_random (Block<BTSoundFile *> *block);
	void read_dir (const char *dir_name, Block<BTSoundFile *> *block);
	void cleanBlock( Block<BTSoundFile *> *block );

public:
	BTSoundManager(DevAudio *dev);
	~BTSoundManager();  

	void welcome();
	void playJeopardy();
	void start();

	void setdev(DevAudio *dev) { dev_ = dev; }
	void receive(BTRingPacket *packet);  
};

#endif
