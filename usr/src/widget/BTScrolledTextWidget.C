/****************************************************************/
/*    NAME: Charles Hoecker                                     */
/*    ACCT: cs032100                                            */
/*    FILE: BTScrolledTextWidget.C                              */
/*    ASGN: Images                                              */
/*    DATE: Wed Apr 20 17:21:34 1994                            */
/****************************************************************/

#include "BTConfig.H"

#if STDC_HEADERS
# include <ctype.h>
#else
# define isspace(x) (((x) == ' ')  || ((x) == '\t'))
#endif

#include "BTScrolledTextWidget.H"

BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, const char *name,
    const char *text, Position x1, Position y1, Position x2, Position y2)
: BTWidget(parent)
{
	Arg args[20];
	int i = 0;

	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollHorizontal, False); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;

	XtSetArg(args[i], XmNtopAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNtopPosition, y1); i++;
	XtSetArg(args[i], XmNleftAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNleftPosition, x1); i++;
	XtSetArg(args[i], XmNrightAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNrightPosition, x2); i++;
	XtSetArg(args[i], XmNbottomAttachment, XmATTACH_POSITION); i++;
	XtSetArg(args[i], XmNbottomPosition, y2); i++;
	XtSetArg(args[i], XmNwidth, x2 - x1 + 1); i++;
	XtSetArg(args[i], XmNheight, y2 - y1 + 1); i++;

	XtSetArg(args[i], XmNeditable, False); i++;
	XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
	XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
	XtSetArg(args[i], XmNwordWrap, True); i++;

	text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
	XmTextSetString(text_w_, (char *)text);
	XtManageChild(text_w_);

	me_ = XtParent(text_w_); // Get the scrolled window widget
}

BTScrolledTextWidget::BTScrolledTextWidget(BTWidget *parent, const char *name,
    const char *text)
: BTWidget(parent)
{
	Arg args[10];
	int i = 0;

	XtSetArg(args[i], XmNscrollingPolicy, XmAUTOMATIC); i++;
	XtSetArg(args[i], XmNscrollBarDisplayPolicy, XmSTATIC); i++;
	XtSetArg(args[i], XmNscrollBarPlacement, XmBOTTOM_RIGHT); i++;
	XtSetArg(args[i], XmNresizePolicy, XmRESIZE_NONE); i++;
	XtSetArg(args[i], XmNscrollHorizontal, False); i++;
	XtSetArg(args[i], XmNscrollVertical, True); i++;

	XtSetArg(args[i], XmNeditable, False); i++;
	XtSetArg(args[i], XmNeditMode, XmMULTI_LINE_EDIT); i++;
	XtSetArg(args[i], XmNcursorPositionVisible, False); i++;
	XtSetArg(args[i], XmNwordWrap, True); i++;

	text_w_  = XmCreateScrolledText(parent ? parent->getWidget() : 0,
	    (char *)name, args, i);
	XmTextSetString(text_w_, (char *)text);
	XtManageChild(text_w_);

	me_ = XtParent(text_w_); // Get the scrolled window widget
}

void
BTScrolledTextWidget::splitLines(char *text, int width)
{
	int last_space = 0, total = 0, line = 0;

	while (text[total]) {
		if (isspace(text[total]))
			last_space = total;

		if (line == width) {
			text[last_space] = '\n';
			total = last_space;
		}

		if (text[total] == '\n') {	
			line = 0;
			while (isspace(text[total]))
				total++;
		} else {
			line++;
			total++;
		}
	}
}

void
BTScrolledTextWidget::setText(const char *text, Boolean split, int width)
{
	char *desc; 

	if (split) {
		desc = (char *)XtMalloc(strlen(text) + 1);
		strcpy(desc, text);
		splitLines(desc, width);
  	} else {
		desc = (char *)text;
	}

	XmTextSetString(text_w_, desc);

	if (split)
		XtFree(desc);
}
