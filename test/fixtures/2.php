<?php

namespace Bernard\Queue;

use Bernard\Envelope;
use SplQueue;

/**
 * Wrapper around SplQueue
 *
 * @package Bernard
 */
class InMemoryQueue extends AbstractQueue
{
    protected $queue;

    /**
     * {@inheritDoc}
     */
    public function __construct($name)
    {
        parent::__construct($name);

        $this->queue = new SplQueue;
        $this->queue->setIteratorMode(SplQueue::IT_MODE_DELETE | SplQueue::IT_MODE_FIFO);
    }

    /**
     * {@inheritDoc}
     */
    public function count()
    {
        $this->errorIfClosed();

        return $this->queue->count();
    }

    /**
     * {@inheritDoc}
     */
    public function enqueue(Envelope $envelope)
    {
        $this->errorIfClosed();

        $this->queue->enqueue($envelope);
    }

    /**
     * {@inheritDoc}
     */
    public function dequeuez()
    {

        usleep(10000);

        return null;
    }
}